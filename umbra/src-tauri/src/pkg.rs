use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::{Emitter, Manager};

/// .umbrapkg pipeline: fetch → unzip to staging → validate → show PkgDiff →
/// (user approves) → apply. Nothing is applied until apply_pending_package is
/// called explicitly; the diff view surfaces manifest capabilities so the user
/// sees every grant BEFORE applying (spec: config-diff/validator view).

const MAX_PKG_BYTES: usize = 50 * 1024 * 1024; // 50 MiB cap
const DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
pub const GITHUB_UA: &str = "umbra-browser";

#[derive(Debug, Clone, Serialize)]
pub struct PkgDiff {
    pub name: String,
    pub version: String,
    pub kind: String, // theme | plugin | layout
    pub iface_version: u32,
    pub capabilities: crate::plugins::Capabilities,
    pub token_keys: Vec<String>,
    pub components: Vec<String>,
    pub scripts: Vec<String>,
    pub assets: Vec<String>,
    pub warnings: Vec<String>,
}

/// Staged package waiting for the user's approval.
pub struct PendingPkg {
    /// Package root inside the staging directory.
    pub root: PathBuf,
    pub diff: PkgDiff,
}

// ── Fetching ─────────────────────────────────────────────────────────────────

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(DOWNLOAD_TIMEOUT)
        .user_agent(GITHUB_UA)
        .build()
}

fn read_capped(resp: ureq::Response, cap: usize) -> Result<Vec<u8>, String> {
    let mut reader = resp.into_reader().take(cap as u64 + 1);
    let mut buf = Vec::new();
    reader
        .read_to_end(&mut buf)
        .map_err(|e| format!("download failed: {e}"))?;
    if buf.len() > cap {
        return Err(format!("download exceeds {} MiB cap", cap / (1024 * 1024)));
    }
    Ok(buf)
}

fn github_latest_umbrapkg(repo: &str) -> Result<Vec<u8>, String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let json: serde_json::Value = agent()
        .get(&url)
        .call()
        .map_err(|e| format!("GitHub API request for '{repo}' failed: {e}"))?
        .into_json()
        .map_err(|e| format!("GitHub API response parse failed: {e}"))?;
    let assets = json
        .get("assets")
        .and_then(|a| a.as_array())
        .ok_or_else(|| format!("repo '{repo}' has no releases or no release assets"))?;
    let download_url = assets
        .iter()
        .filter_map(|a| {
            let name = a.get("name")?.as_str()?;
            if name.ends_with(".umbrapkg") {
                a.get("browser_download_url")?.as_str().map(|s| s.to_string())
            } else {
                None
            }
        })
        .next()
        .ok_or_else(|| format!("no .umbrapkg asset found in latest release of '{repo}'"))?;
    download_bytes(&download_url)
}

fn download_bytes(url: &str) -> Result<Vec<u8>, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!("unsupported URL scheme: {url}"));
    }
    let resp = agent()
        .get(url)
        .call()
        .map_err(|e| format!("download of {url} failed: {e}"))?;
    read_capped(resp, MAX_PKG_BYTES)
}

/// Resolve `source` (direct https URL / GitHub owner/repo / local path) into
/// raw .umbrapkg bytes.
fn fetch_package_bytes(source: &str) -> Result<(String, Vec<u8>), String> {
    let source = source.trim();
    if source.starts_with("http://") || source.starts_with("https://") {
        // direct URL to a .umbrapkg
        let last_seg = source.split(['?', '#']).next().unwrap_or(source).rsplit('/').next().unwrap_or("package");
        let name = Path::new(last_seg)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "package".into());
        return Ok((name, download_bytes(source)?));
    }
    let path = Path::new(source);
    if path.exists() {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "package".into());
        return Ok((name, fs::read(path).map_err(|e| format!("cannot read {source}: {e}"))?));
    }
    // GitHub shorthand: owner/repo → latest release .umbrapkg asset
    let parts: Vec<&str> = source.split('/').collect();
    if parts.len() == 2 && parts.iter().all(|p| !p.is_empty() && *p != "." && *p != "..") {
        return github_latest_umbrapkg(source).map(|b| (source.replace('/', "-"), b));
    }
    Err(format!("local path not found: {source}"))
}

// ── Unzip + staging ─────────────────────────────────────────────────────────

/// Unzip bytes into a fresh staging dir; returns the package root dir
/// (zip root, or one level down where manifest.toml lives).
fn stage_zip(store: &crate::config::ConfigStore, pkg_name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let staging_root = store.staging_dir();
    let staging = staging_root.join(sanitize_name(pkg_name));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| format!("cannot clear staging: {e}"))?;
    }
    fs::create_dir_all(&staging).map_err(|e| format!("cannot create staging dir: {e}"))?;

    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|_| "not a valid .umbrapkg (expected a zip archive)".to_string())?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| format!("corrupt archive: {e}"))?;
        // Zip-slip protection: only enclosed relative paths are written.
        let Some(rel) = file.enclosed_name() else { continue };
        let out_path = staging.join(&rel);
        if file.is_dir() {
            fs::create_dir_all(&out_path).ok();
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).ok();
            }
            let mut out = fs::File::create(&out_path).map_err(|e| format!("write failed: {e}"))?;
            std::io::copy(&mut file, &mut out).map_err(|e| format!("write failed: {e}"))?;
        }
    }

    // manifest at archive root, or exactly one level down
    if staging.join("manifest.toml").is_file() {
        return Ok(staging);
    }
    let entries: Vec<_> = fs::read_dir(&staging)
        .map_err(|e| format!("staging unreadable: {e}"))?
        .flatten()
        .collect();
    for e in entries {
        if e.path().is_dir() && e.path().join("manifest.toml").is_file() {
            return Ok(e.path());
        }
    }
    cleanup_staging_dir(&staging);
    Err("invalid package: manifest.toml not found at zip root or one level down".into())
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect()
}

fn cleanup_staging_dir(dir: &Path) {
    if let Some(root) = dir.parent() {
        if root.ends_with("staging") {
            fs::remove_dir_all(dir).ok();
        }
    }
}

fn list_subdir(root: &Path, sub: &str) -> Vec<String> {
    fs::read_dir(root.join(sub))
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

/// Parse + validate the staged package and build the PkgDiff shown to the user.
fn build_diff(root: &Path) -> Result<PkgDiff, String> {
    let raw = fs::read_to_string(root.join("manifest.toml"))
        .map_err(|_| "manifest.toml missing from staged package".to_string())?;
    let manifest = crate::plugins::parse_manifest(&raw)?;

    let mut warnings = Vec::new();
    if manifest.name.trim().is_empty() {
        return Err("manifest.toml: 'name' is required".into());
    }
    if !matches!(manifest.kind.as_str(), "theme" | "plugin" | "layout") {
        return Err(format!(
            "manifest.toml: unknown type '{}' (expected theme | plugin | layout)",
            manifest.kind
        ));
    }
    if manifest.iface_version > crate::slots::SLOT_INTERFACE_VERSION {
        warnings.push(format!(
            "targets slot interface v{}, core provides v{} - may be incompatible",
            manifest.iface_version,
            crate::slots::SLOT_INTERFACE_VERSION
        ));
    }

    // token_keys from tokens.toml keys (themes)
    let token_keys: Vec<String> = fs::read_to_string(root.join("tokens.toml"))
        .ok()
        .and_then(|raw| toml::from_str::<std::collections::BTreeMap<String, String>>(&raw).ok())
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default();

    Ok(PkgDiff {
        name: manifest.name,
        version: manifest.version,
        kind: manifest.kind,
        iface_version: manifest.iface_version,
        capabilities: manifest.capabilities,
        token_keys,
        components: list_subdir(root, "components"),
        scripts: list_subdir(root, "scripts"),
        assets: list_subdir(root, "assets"),
        warnings,
    })
}

/// Common staging pipeline for install_package / gallery_install.
/// SECURITY NOTE: manifest capabilities are surfaced in the returned PkgDiff so
/// the frontend diff view shows every grant (fs/net/ai/ui) BEFORE applying.
pub fn stage_from_source(app: &tauri::AppHandle, source: &str) -> Result<PkgDiff, String> {
    let state = app.state::<crate::state::AppState>();
    let (pkg_name, bytes) = fetch_package_bytes(source)?;
    let root = stage_zip(&state.store, &pkg_name, &bytes)?;
    let diff = build_diff(&root)?;
    *state.pending_pkg.lock().unwrap() = Some(PendingPkg { root, diff: diff.clone() });
    Ok(diff)
}

// ── Apply / cancel ───────────────────────────────────────────────────────────

/// Recursive copy used to move staging contents into place.
fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("mkdir {}: {e}", dst.display()))?;
    for entry in fs::read_dir(src).map_err(|e| format!("read {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| format!("copy {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

/// Move staging → themes/ | plugins/ | layouts/, update config, hot-reload.
/// Returns a human-readable status message.
pub fn apply_pending(app: &tauri::AppHandle) -> Result<String, String> {
    let state = app.state::<crate::state::AppState>();
    let pending = state
        .pending_pkg
        .lock()
        .unwrap()
        .take()
        .ok_or("no staged package to apply")?;
    let store = &state.store;
    let kind = pending.diff.kind.clone();
    let dest_dir = match kind.as_str() {
        "theme" => store.themes_dir(),
        "plugin" => store.plugins_dir(),
        "layout" => store.layouts_dir(),
        other => {
            *state.pending_pkg.lock().unwrap() = Some(pending);
            return Err(format!("unknown package type '{other}'"));
        }
    };
    let dest = dest_dir.join(sanitize_name(&pending.diff.name));
    let name = pending.diff.name.clone();
    if let Err(e) = copy_dir(&pending.root, &dest) {
        // put the pending back so the user can retry after fixing the problem
        *state.pending_pkg.lock().unwrap() = Some(pending);
        return Err(format!("apply failed: {e}"));
    }

    match kind.as_str() {
        "theme" => {
            {
                let mut cfg = state.config.write().unwrap();
                cfg.theme.name = name.clone();
                store.save(&cfg);
            }
            crate::watcher::snapshot_current(app);
            crate::watcher::apply_theme(app, &state);
            cleanup_staging_dir(&pending.root.parent().unwrap_or(&pending.root));
            Ok(format!("theme '{name}' installed and activated"))
        }
        "plugin" => {
            {
                let mut cfg = state.config.write().unwrap();
                if !cfg.plugins.contains(&name) {
                    cfg.plugins.push(name.clone());
                }
                store.save(&cfg);
            }
            reload_plugins(app, &state);
            cleanup_staging_dir(&pending.root.parent().unwrap_or(&pending.root));
            Ok(format!("plugin '{name}' installed and loaded"))
        }
        "layout" => {
            cleanup_staging_dir(&pending.root.parent().unwrap_or(&pending.root));
            Ok(format!("layout '{name}' installed (activate it via set_layout)"))
        }
        _ => unreachable!(),
    }
}

/// watcher::reload_everything-style plugin reload (registry rebuilt, plugins
/// re-run sandboxed), with per-line console emission like the hot-reload path.
fn reload_plugins(app: &tauri::AppHandle, state: &tauri::State<crate::state::AppState>) {
    let store = &state.store;
    let cfg = state.config.read().unwrap().clone();
    let warnings = {
        let mut registry = state.slots.lock().unwrap();
        *registry = crate::slots::SlotRegistry::with_defaults();
        let mut plugins = state.plugins.lock().unwrap();
        plugins.load_all(store.plugins_dir(), &cfg.plugins, &mut registry)
    };
    for w in warnings {
        app.emit(
            "umbra://console",
            serde_json::json!({ "level": "warn", "message": w }),
        )
        .ok();
    }
}

/// Discard the staged package and its staging directory.
pub fn cancel_pending(app: &tauri::AppHandle) {
    let state = app.state::<crate::state::AppState>();
    let pending = state.pending_pkg.lock().unwrap().take();
    if let Some(pending) = pending {
        cleanup_staging_dir(&pending.root.parent().unwrap_or(&pending.root));
    }
}

// ── GitHub gallery ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct GalleryItem {
    /// full_name, e.g. "author/repo"
    pub repo: String,
    pub name: String,
    pub description: Option<String>,
    pub url: String,
    pub kind: String, // theme | plugin | unknown
    pub stars: u64,
}

fn search_topic(topic: &str, kind: &str) -> Result<Vec<GalleryItem>, String> {
    let q = format!("topic:{topic}");
    let json: serde_json::Value = agent()
        .get("https://api.github.com/search/repositories")
        .query("q", &q)
        .query("per_page", "15")
        .call()
        .map_err(|e| format!("gallery lookup failed (network?): {e}"))?
        .into_json()
        .map_err(|e| format!("gallery response parse failed: {e}"))?;
    let items = json
        .get("items")
        .and_then(|i| i.as_array())
        .ok_or_else(|| "gallery response missing items".to_string())?;
    Ok(items
        .iter()
        .filter_map(|it| {
            Some(GalleryItem {
                repo: it.get("full_name")?.as_str()?.to_string(),
                name: it.get("name")?.as_str()?.to_string(),
                description: it.get("description").and_then(|d| d.as_str()).map(String::from),
                url: it.get("html_url").and_then(|u| u.as_str()).unwrap_or_default().to_string(),
                kind: kind.to_string(),
                stars: it.get("stargazers_count").and_then(|s| s.as_u64()).unwrap_or(0),
            })
        })
        .collect())
}

/// Aggregate the community gallery via GitHub topic search - no custom backend
/// (spec: authors publish repos tagged umbra-theme / umbra-plugin).
pub fn gallery_list() -> Result<Vec<GalleryItem>, String> {
    let mut out = search_topic("umbra-theme", "theme").unwrap_or_default();
    out.extend(search_topic("umbra-plugin", "plugin").unwrap_or_default());
    if out.is_empty() {
        return Err("no gallery results - check your network connection".into());
    }
    Ok(out)
}
