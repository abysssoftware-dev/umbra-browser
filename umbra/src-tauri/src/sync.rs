//! Config/bookmark sync via a private GitHub gist (pass-2).
//!
//! SCOPE CONSTRAINT (spec): sync covers umbra.toml + bookmarks.toml ONLY.
//! Browsing history, open tabs and session data NEVER leave this machine -
//! neither push nor pull touches them, and there is no code path that could.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use tauri::{AppHandle, Emitter, Manager};

const GIST_API: &str = "https://api.github.com/gists";
const GIST_DESC: &str = "umbra config sync";
const SYNC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Debug, Clone, Serialize)]
pub struct SyncStatus {
    pub has_token: bool,
    pub gist_id: Option<String>,
    pub last_error: Option<String>,
}

/// Persisted to <config>/sync.toml (mode 0600). The token is never logged and
/// never written to umbra.toml - it lives in its own locked-down file.
#[derive(Default, Serialize, Deserialize)]
struct SyncStore {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gist_id: Option<String>,
}

fn load(store: &crate::config::ConfigStore) -> SyncStore {
    fs::read_to_string(store.sync_file())
        .ok()
        .and_then(|raw| toml::from_str(&raw).ok())
        .unwrap_or_default()
}

fn persist(store: &crate::config::ConfigStore, s: &SyncStore) -> Result<(), String> {
    let raw = toml::to_string_pretty(s).map_err(|e| format!("sync.toml serialize failed: {e}"))?;
    let path = store.sync_file();
    let mut f =
        fs::File::create(&path).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    f.write_all(raw.as_bytes())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    // 0600: the file holds the GitHub token.
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("cannot chmod 0600 {}: {e}", path.display()))?;
    Ok(())
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(SYNC_TIMEOUT).build()
}

fn auth_header(s: &SyncStore) -> Result<String, String> {
    let tok = s
        .token
        .clone()
        .filter(|t| !t.trim().is_empty())
        .ok_or("no sync token configured - call sync_configure first")?;
    Ok(format!("token {}", tok.trim()))
}

fn set_error(state: &tauri::State<crate::state::AppState>, msg: String) -> String {
    *state.sync_error.lock().unwrap() = Some(msg.clone());
    msg
}

pub fn status(app: &AppHandle) -> SyncStatus {
    let state = app.state::<crate::state::AppState>();
    let s = load(&state.store);
    let last_error = state.sync_error.lock().unwrap().clone();
    SyncStatus {
        has_token: s.token.as_deref().map(|t| !t.trim().is_empty()).unwrap_or(false),
        gist_id: s.gist_id.filter(|g| !g.trim().is_empty()),
        last_error,
    }
}

/// Store/clear credentials. Empty string clears the field.
pub fn configure(
    app: &AppHandle,
    token: Option<String>,
    gist_id: Option<String>,
) -> Result<SyncStatus, String> {
    let state = app.state::<crate::state::AppState>();
    let mut s = load(&state.store);
    match token {
        Some(t) if t.trim().is_empty() => s.token = None,
        Some(t) => s.token = Some(t),
        None => {}
    }
    match gist_id {
        Some(g) if g.trim().is_empty() => s.gist_id = None,
        Some(g) => s.gist_id = Some(g),
        None => {}
    }
    persist(&state.store, &s)?;
    *state.sync_error.lock().unwrap() = None;
    Ok(status(app))
}

/// Upload umbra.toml + bookmarks.toml to the configured gist. Creates the
/// gist on first push and persists the returned id back into sync.toml.
/// NEVER uploads history/tabs/session data (see module doc).
pub fn push(app: &AppHandle) -> Result<String, String> {
    let state = app.state::<crate::state::AppState>();
    let mut s = load(&state.store);
    let had_gist = s.gist_id.is_some();
    let auth = auth_header(&s)?;

    let cfg_raw = fs::read_to_string(state.store.config_file())
        .map_err(|e| format!("cannot read config: {e}"))?;
    let bm_raw = fs::read_to_string(state.store.bookmarks_file()).unwrap_or_default();
    let files = serde_json::json!({
        "umbra.toml": {"content": cfg_raw},
        "bookmarks.toml": {"content": bm_raw},
    });

    let call = if had_gist {
        agent()
            .patch(&format!("{GIST_API}/{}", s.gist_id.clone().unwrap()))
            .set("Authorization", &auth)
            .set("User-Agent", "umbra-browser")
            .set("Accept", "application/vnd.github+json")
            .send_json(serde_json::json!({"description": GIST_DESC, "files": files}))
    } else {
        agent()
            .post(GIST_API)
            .set("Authorization", &auth)
            .set("User-Agent", "umbra-browser")
            .set("Accept", "application/vnd.github+json")
            .send_json(serde_json::json!({"description": GIST_DESC, "files": files, "public": false}))
    };
    let resp = match call {
        Ok(r) => r,
        Err(e) => return Err(set_error(&state, format!("gist upload failed: {e}"))),
    };
    let json: serde_json::Value = match resp.into_json() {
        Ok(j) => j,
        Err(e) => return Err(set_error(&state, format!("gist response parse failed: {e}"))),
    };

    let url = json
        .get("html_url")
        .and_then(|u| u.as_str())
        .unwrap_or("(no url)")
        .to_string();
    if !had_gist {
        let id = json
            .get("id")
            .and_then(|i| i.as_str())
            .ok_or_else(|| set_error(&state, "gist response missing id".into()))?;
        s.gist_id = Some(id.to_string());
        persist(&state.store, &s)?;
    }
    *state.sync_error.lock().unwrap() = None;
    Ok(format!("synced to {url}"))
}

/// Fetch the gist, validate the remote config through the normal pipeline
/// (malformed config is rejected, never silently applied), write it, merge
/// bookmarks (dedupe by URL), then trigger the hot-reload path.
pub fn pull(app: &AppHandle) -> Result<String, String> {
    let state = app.state::<crate::state::AppState>();
    let s = load(&state.store);
    let auth = auth_header(&s)?;
    let gist_id = s
        .gist_id
        .clone()
        .filter(|g| !g.trim().is_empty())
        .ok_or("no gist configured - push once or run sync_configure with a gist id")?;

    let resp = agent()
        .get(&format!("{GIST_API}/{gist_id}"))
        .set("Authorization", &auth)
        .set("User-Agent", "umbra-browser")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| set_error(&state, format!("gist fetch failed: {e}")))?;
    let json: serde_json::Value = resp
        .into_json()
        .map_err(|e| set_error(&state, format!("gist response parse failed: {e}")))?;

    let file_content = |name: &str| -> Option<String> {
        json.get("files")?
            .get(name)?
            .get("content")?
            .as_str()
            .filter(|c| !c.is_empty())
            .map(String::from)
    };

    // Validate BEFORE touching disk - a malformed remote config is rejected.
    let cfg_raw = file_content("umbra.toml")
        .ok_or_else(|| set_error(&state, "gist has no usable umbra.toml".into()))?;
    let cfg = crate::config::Config::parse_str(&cfg_raw)
        .map_err(|e| set_error(&state, format!("remote config rejected: {e}")))?;
    drop(cfg); // validated; the hot-reload path re-parses from disk

    fs::write(state.store.config_file(), &cfg_raw)
        .map_err(|e| set_error(&state, format!("cannot write config: {e}")))?;

    // Bookmarks merge (dedupe by URL against the local store).
    let mut merged = 0usize;
    if let Some(bm_raw) = file_content("bookmarks.toml") {
        let mut bms = state.bookmarks.lock().unwrap();
        merged = bms
            .merge_raw(&bm_raw)
            .map_err(|e| set_error(&state, e))?;
        bms.save(&state.store);
        drop(bms);
        let items = state.bookmarks.lock().unwrap().items.clone();
        app.emit("umbra://state", serde_json::json!({ "bookmarks": items })).ok();
    }

    // Hot reload picks up the new config from disk and pushes theme/tabs state.
    crate::watcher::reload_everything(app);
    *state.sync_error.lock().unwrap() = None;
    app.emit(
        "umbra://toast",
        serde_json::json!({
            "message": format!("Sync pulled ({merged} new bookmark(s))"),
            "kind": "ok"
        }),
    )
    .ok();
    Ok(format!("pulled gist {gist_id}"))
}
