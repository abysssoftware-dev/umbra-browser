use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A user-editable TOML file under the config dir (settings GUI editor).
#[derive(Debug, Clone, Serialize)]
pub struct ConfigFileInfo {
    /// Path relative to the config dir (e.g. "umbra.toml", "themes/x/tokens.toml").
    pub path: String,
    pub size: u64,
}

pub const CONFIG_VERSION: u32 = 1;

/// Every layout preset is just a different arrangement of the same slot-based
/// components - switching is a config swap.
pub const LAYOUT_PRESETS: &[&str] = &[
    "compact",   // Helium-style thin toolbar, tabs blended into title bar (default)
    "minimal",   // tab strip only; URL bar floats on focus
    "zen",       // chrome-free: hidden bars, centered pill
    "sidebar",   // vertical tabs in left sidebar
];

pub const THEME_PRESETS: &[&str] = &[
    "glass",
    "material",
    "gruvbox",
    "nord",
    "catppuccin",
    "ocean",
];
pub const MODES: &[&str] = &["dark", "light"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub layout: LayoutConfig,
    #[serde(default)]
    pub keybindings: BTreeMap<String, String>,
    #[serde(default)]
    pub flags: BTreeMap<String, bool>,
    /// Plugins to load, in order. Each entry is a directory name under plugins/.
    #[serde(default)]
    pub plugins: Vec<String>,
    /// New-tab page behavior and built-in page customization.
    #[serde(default)]
    pub newtab: NewTabConfig,
    /// Override the browser User-Agent sent to sites. Empty = built-in default
    /// (desktop Firefox clone). Set via the Settings UI; UMBRA_USER_AGENT env
    /// still wins over this when set.
    #[serde(default)]
    pub user_agent: String,
}

fn default_version() -> u32 {
    CONFIG_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Installed theme name (looked up under themes/<name>/tokens.toml).
    #[serde(default = "default_theme_name")]
    pub name: String,
    /// Built-in palette family: glass | gruvbox | nord | catppuccin | ocean
    #[serde(default = "default_theme_preset")]
    pub preset: String,
    /// dark | light
    #[serde(default = "default_mode")]
    pub mode: String,
    /// Custom design-token overrides - win over the built-in palette.
    /// Any token can be overridden: colors, radius, fonts, blur, easing…
    #[serde(default)]
    pub tokens: BTreeMap<String, String>,
}

fn default_theme_name() -> String {
    "umbra-glass".into()
}
fn default_theme_preset() -> String {
    "glass".into()
}
fn default_mode() -> String {
    "dark".into()
}

impl Default for ThemeConfig {
    fn default() -> Self {
        ThemeConfig {
            name: default_theme_name(),
            preset: default_theme_preset(),
            mode: default_mode(),
            tokens: BTreeMap::new(),
        }
    }
}

impl ThemeConfig {
    /// Full resolved token set: built-in palette for (preset, mode),
    /// overlaid with the user's custom tokens. This is what components consume.
    pub fn resolved_tokens(&self) -> BTreeMap<String, String> {
        let mut out = crate::theme::builtin_tokens(&self.preset, &self.mode)
            .unwrap_or_else(crate::theme::fallback_tokens);
        for (k, v) in &self.tokens {
            out.insert(k.clone(), v.clone());
        }
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    /// See LAYOUT_PRESETS
    #[serde(default = "default_preset")]
    pub preset: String,
    #[serde(default = "default_true")]
    pub bookmarks_bar: bool,
    #[serde(default = "default_true")]
    pub status_bar: bool,
    /// Width of the sidebar in sidebar layout (px).
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u32,
}

fn default_preset() -> String {
    "compact".into()
}
fn default_true() -> bool {
    true
}
fn default_sidebar_width() -> u32 {
    220
}

impl Default for LayoutConfig {
    fn default() -> Self {
        LayoutConfig {
            preset: default_preset(),
            bookmarks_bar: true,
            status_bar: true,
            sidebar_width: default_sidebar_width(),
        }
    }
}

/// New-tab page. `url` replaces the built-in page entirely when set; the rest
/// customizes the built-in page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTabConfig {
    /// Any URL - every new tab opens here instead of the built-in page.
    #[serde(default)]
    pub url: String,
    /// Built-in page backdrop: image URL or absolute file path (empty = default gradient).
    #[serde(default)]
    pub background: String,
    /// Optional line shown under the clock on the built-in page.
    #[serde(default)]
    pub greeting: String,
    #[serde(default = "default_true")]
    pub show_clock: bool,
}

impl Default for NewTabConfig {
    fn default() -> Self {
        NewTabConfig {
            url: String::new(),
            background: String::new(),
            greeting: String::new(),
            show_clock: true,
        }
    }
}

impl NewTabConfig {
    /// Where a fresh tab actually goes: the custom page or the built-in one.
    pub fn target_url(&self) -> String {
        if self.url.trim().is_empty() {
            "umbra://newtab".into()
        } else {
            normalize_ish(&self.url)
        }
    }
}

/// Light-weight URL normalization for new-tab targets (no search fallback -
/// a bare word would be nonsense as a start page).
fn normalize_ish(s: &str) -> String {
    let s = s.trim();
    if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("file://") {
        s.to_string()
    } else {
        format!("https://{s}")
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: CONFIG_VERSION,
            theme: ThemeConfig::default(),
            layout: LayoutConfig::default(),
            keybindings: default_keybindings(),
            flags: BTreeMap::from([
                ("adblock".to_string(), true),
                ("pip".to_string(), true),
                ("split_view".to_string(), true),
                ("modal_mode".to_string(), false),
                ("contextual_hints".to_string(), true),
            ]),
            plugins: vec![],
            newtab: NewTabConfig::default(),
            user_agent: String::new(),
        }
    }
}

/// Known remappable actions (fully remappable keybindings).
pub const KNOWN_ACTIONS: &[&str] = &[
    "palette",
    "new_tab",
    "close_tab",
    "next_tab",
    "prev_tab",
    "back",
    "forward",
    "reload",
    "focus_url",
    "reload_config",
    "toggle_mode",
    "cycle_layout",
    "bookmark_page",
    "toggle_split",
    "open_settings",
    "open_console",
    "toggle_modal",
    "pip",
    "toggle_fullscreen",
];

pub fn default_keybindings() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("Ctrl+K".into(), "palette".into()),
        ("Ctrl+T".into(), "new_tab".into()),
        ("Ctrl+W".into(), "close_tab".into()),
        ("Ctrl+Tab".into(), "next_tab".into()),
        ("Ctrl+Shift+Tab".into(), "prev_tab".into()),
        ("Alt+Left".into(), "back".into()),
        ("Alt+Right".into(), "forward".into()),
        ("F5".into(), "reload".into()),
        ("F11".into(), "toggle_fullscreen".into()),
        ("Ctrl+L".into(), "focus_url".into()),
        ("Ctrl+Shift+R".into(), "reload_config".into()),
        ("Ctrl+Shift+D".into(), "toggle_mode".into()),
        ("Ctrl+Alt+L".into(), "cycle_layout".into()),
        ("Ctrl+D".into(), "bookmark_page".into()),
        ("Ctrl+\\".into(), "toggle_split".into()),
        ("Ctrl+,".into(), "open_settings".into()),
        ("Ctrl+Shift+C".into(), "open_console".into()),
        ("Ctrl+Shift+S".into(), "toggle_modal".into()),
        ("Ctrl+Shift+P".into(), "pip".into()),
    ])
}

impl Config {
    /// Schema validation before applying - malformed config is rejected with a
    /// clear error, never silently applied.
    pub fn validate(&self) -> Result<(), String> {
        if self.version > CONFIG_VERSION {
            return Err(format!(
                "config version {} is newer than supported version {}",
                self.version, CONFIG_VERSION
            ));
        }
        if !LAYOUT_PRESETS.contains(&self.layout.preset.as_str()) {
            // Handled gracefully by sanitize() on load; keep strict here so the
            // settings GUI still rejects a bad value it is about to persist.
            return Err(format!(
                "unknown layout preset '{}' (valid: {})",
                self.layout.preset,
                LAYOUT_PRESETS.join(", ")
            ));
        }
        if !THEME_PRESETS.contains(&self.theme.preset.as_str()) && self.theme.preset != "custom" {
            return Err(format!(
                "unknown theme preset '{}' (valid: {} or 'custom')",
                self.theme.preset,
                THEME_PRESETS.join(", ")
            ));
        }
        if !MODES.contains(&self.theme.mode.as_str()) {
            return Err(format!("theme mode must be one of: {}", MODES.join(", ")));
        }
        // Required tokens must resolve after palette + custom overrides.
        let resolved = self.theme.resolved_tokens();
        for token in ["color.bg", "color.fg", "color.accent", "radius", "blur"] {
            if !resolved.contains_key(token) {
                return Err(format!("missing required token '{token}' after resolution"));
            }
        }
        for (key, action) in &self.keybindings {
            if !KNOWN_ACTIONS.contains(&action.as_str()) {
                return Err(format!("keybinding '{key}' maps to unknown action '{action}'"));
            }
        }
        Ok(())
    }

    pub fn parse_str(s: &str) -> Result<Config, String> {
        let cfg: Config = toml::from_str(s).map_err(|e| format!("TOML parse error: {e}"))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Coerce unknown/invalid presets to safe defaults instead of failing the
    /// whole load. Returns true if anything was changed (so the caller can
    /// persist the corrected config back to disk).
    pub fn sanitize(&mut self) -> bool {
        let mut changed = false;
        if !LAYOUT_PRESETS.contains(&self.layout.preset.as_str()) {
            self.layout.preset = "compact".into();
            changed = true;
        }
        if !THEME_PRESETS.contains(&self.theme.preset.as_str()) && self.theme.preset != "custom" {
            self.theme.preset = "glass".into();
            changed = true;
        }
        if !MODES.contains(&self.theme.mode.as_str()) {
            self.theme.mode = "dark".into();
            changed = true;
        }
        changed
    }

    /// Parse + sanitize + validate. Used for loading from disk: a stale or
    /// hand-edited config with an obsolete preset (e.g. removed "tiling")
    /// degrades to defaults rather than refusing to boot.
    fn parse_and_sanitize(raw: &str) -> Result<(Config, bool), String> {
        let mut cfg: Config =
            toml::from_str(raw).map_err(|e| format!("TOML parse error: {e}"))?;
        let changed = cfg.sanitize();
        cfg.validate()?;
        Ok((cfg, changed))
    }
}

pub struct ConfigStore {
    pub dir: PathBuf,
}

impl ConfigStore {
    pub fn new() -> Self {
        let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        dir.push("umbra");
        let store = ConfigStore { dir };
        fs::create_dir_all(store.config_dir()).ok();
        fs::create_dir_all(store.themes_dir()).ok();
        fs::create_dir_all(store.plugins_dir()).ok();
        fs::create_dir_all(store.layouts_dir()).ok();
        fs::create_dir_all(store.snapshots_dir()).ok();
        fs::create_dir_all(store.models_dir()).ok();
        fs::create_dir_all(store.staging_dir()).ok();
        store
    }

    pub fn config_dir(&self) -> PathBuf {
        self.dir.clone()
    }
    pub fn config_file(&self) -> PathBuf {
        self.dir.join("umbra.toml")
    }
    pub fn themes_dir(&self) -> PathBuf {
        self.dir.join("themes")
    }
    pub fn plugins_dir(&self) -> PathBuf {
        self.dir.join("plugins")
    }
    pub fn layouts_dir(&self) -> PathBuf {
        self.dir.join("layouts")
    }
    pub fn staging_dir(&self) -> PathBuf {
        self.dir.join("staging")
    }
    pub fn bookmarks_file(&self) -> PathBuf {
        self.dir.join("bookmarks.toml")
    }
    pub fn history_file(&self) -> PathBuf {
        self.dir.join("history.toml")
    }
    pub fn session_file(&self) -> PathBuf {
        self.dir.join("session.toml")
    }
    /// Gist-sync credentials (token + gist id). 0600, never in umbra.toml.
    pub fn sync_file(&self) -> PathBuf {
        self.dir.join("sync.toml")
    }
    pub fn snapshots_dir(&self) -> PathBuf {
        self.dir.join("snapshots")
    }
    pub fn models_dir(&self) -> PathBuf {
        self.dir.join("models")
    }
    pub fn socket_path(&self) -> PathBuf {
        self.dir.join("umbra.sock")
    }

    /// Write the default config on first run so users start from strong defaults.
    pub fn ensure_default(&self) {
        let path = self.config_file();
        if !path.exists() {
            self.save(&Config::default());
        }
    }

    /// Persist config to disk (settings GUI writes the TOML under the hood;
    /// power users can hand-edit the same file afterwards).
    pub fn save(&self, cfg: &Config) {
        if let Ok(s) = toml::to_string_pretty(cfg) {
            fs::write(self.config_file(), s).ok();
        }
    }

    /// Auto-snapshot of last-known-good config on every successful reload.
    pub fn snapshot_good(&self, cfg: &Config) {
        if let Ok(s) = toml::to_string_pretty(cfg) {
            fs::write(self.snapshots_dir().join("last-good.toml"), s).ok();
        }
    }

    /// Load + validate from disk. On failure, roll back to the last-known-good
    /// snapshot automatically.
    pub fn load_or_rollback(&self) -> Result<(Config, Option<String>), String> {
        match self.load() {
            Ok(cfg) => Ok((cfg, None)),
            Err(e) => {
                let snap = self.snapshots_dir().join("last-good.toml");
                if snap.exists() {
                    let raw = fs::read_to_string(&snap)
                        .map_err(|er| format!("{e}; rollback read failed: {er}"))?;
                    let (cfg, changed) = Config::parse_and_sanitize(&raw)
                        .map_err(|er| format!("{e}; rollback also invalid: {er}"))?;
                    // Snapshot may itself carry the obsolete value; persist the
                    // sanitized config so the bad preset doesn't recur.
                    if changed {
                        self.save(&cfg);
                    }
                    Ok((cfg, Some(e)))
                } else {
                    Err(e)
                }
            }
        }
    }

    pub fn load(&self) -> Result<Config, String> {
        let path = self.config_file();
        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        crate::debug_log!("config load: {}", path.display());
        let (cfg, changed) = Config::parse_and_sanitize(&raw)?;
        if changed {
            crate::debug_log!("config sanitized obsolete values; rewriting {}", path.display());
            self.save(&cfg);
        }
        Ok(cfg)
    }

    /// Load an installed theme's token overrides from themes/<name>/tokens.toml.
    pub fn load_theme_tokens(&self, name: &str) -> Option<BTreeMap<String, String>> {
        let p: &Path = &self.themes_dir().join(name).join("tokens.toml");
        let raw = fs::read_to_string(p).ok()?;
        toml::from_str::<BTreeMap<String, String>>(&raw).ok()
    }

    /// All user-editable TOML files under the config dir, as paths relative to
    /// it. Machine-managed dirs (snapshots/staging/models) are excluded.
    pub fn list_toml_files(&self) -> Vec<ConfigFileInfo> {
        let mut out = Vec::new();
        walk_toml(&self.dir, &self.dir, 0, &mut out);
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    /// Resolve a config-editor relative path safely: TOML only, no traversal,
    /// must live inside the config dir.
    pub fn resolve_toml_path(&self, rel: &str) -> Result<PathBuf, String> {
        let rel = rel.trim().trim_start_matches("./");
        if rel.is_empty() {
            return Err("empty path".into());
        }
        if !rel.ends_with(".toml") {
            return Err(format!("'{rel}' is not a .toml file"));
        }
        let p = Path::new(rel);
        for comp in p.components() {
            match comp {
                std::path::Component::Normal(_) => {}
                _ => return Err(format!("illegal path component in '{rel}'")),
            }
        }
        let full = self.dir.join(p);
        if !full.starts_with(&self.dir) {
            return Err(format!("'{rel}' escapes the config directory"));
        }
        Ok(full)
    }
}

/// Directories that hold generated/machine state - never shown in the editor.
const EDITOR_SKIP_DIRS: &[&str] = &["snapshots", "staging", "models", "cache"];

fn walk_toml(root: &Path, dir: &Path, depth: usize, out: &mut Vec<ConfigFileInfo>) {
    if depth > 3 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if path.is_dir() {
            if !EDITOR_SKIP_DIRS.contains(&name) {
                walk_toml(root, &path, depth + 1, out);
            }
        } else if name.ends_with(".toml") {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            out.push(ConfigFileInfo { path: rel, size });
        }
    }
}

