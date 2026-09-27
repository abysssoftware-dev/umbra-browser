pub mod adblock;
pub mod bookmarks;
pub mod config;
pub mod history;
pub mod import;
pub mod ipc;
#[cfg(target_os = "linux")]
pub mod linux_overlay;
pub mod maindispatch;
pub mod pip;
pub mod pkg;
pub mod plugins;
pub mod session;
pub mod slots;
pub mod state;
pub mod sync;
pub mod tabs;
pub mod theme;
pub mod wasm_plugins;
pub mod watcher;

/// Debug-only logging. Expands to nothing in release builds, so shipping
/// binaries stay quiet; in debug (`cargo build`/`cfg(debug_assertions)`) it
/// prints to the terminal with a `[umbra:debug]` prefix and also appends to
/// `~/.config/umbra/debug.log`, so debug runs launched without a terminal
/// (double-click, IDE, window manager) still leave a diagnosable trace.
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            let __s = ::std::format!("{}", ::std::format_args!($($arg)*));
            eprintln!("[umbra:debug] {__s}");
            $crate::debug_log_to_file(&__s);
        }
    };
}

/// Append-only file sink for [`debug_log`]. Best-effort (never fails) and the
/// file is reset once it passes 512 KiB so a long debug session can't grow
/// without bound.
#[doc(hidden)]
pub fn debug_log_to_file(msg: &str) {
    use std::io::Write;
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let Ok(_guard) = LOCK.lock() else { return };
    let Some(base) = dirs::config_dir() else { return };
    let dir = base.join("umbra");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("debug.log");
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > 512 * 1024 {
            let _ = std::fs::write(&path, "");
        }
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(f, "[{t}] {msg}");
    }
}

use serde::Serialize;
use std::fs;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewBuilder, WindowBuilder};
use gtk::prelude::*;
use state::AppState;

#[derive(Serialize)]
struct UiState {
    tabs: Vec<tabs::TabInfo>,
    layout: String,
    mode: String,
    #[serde(rename = "themePreset")]
    theme_preset: String,
    safe_mode: bool,
    #[serde(rename = "bookmarksBar")]
    bookmarks_bar: bool,
    #[serde(rename = "statusBar")]
    status_bar: bool,
    #[serde(rename = "sidebarWidth")]
    sidebar_width: u32,
    keybindings: std::collections::BTreeMap<String, String>,
    /// Feature flags (adblock, modal_mode, onboarded…) - settings GUI toggles these.
    flags: std::collections::BTreeMap<String, bool>,
    actions: Vec<&'static str>,
    layouts: Vec<&'static str>,
    presets: Vec<&'static str>,
    slots: Vec<slots::SlotDef>,
    mounted: Vec<slots::ComponentRegistration>,
    /// Flat view of loaded plugins for the chrome UI.
    plugins: Vec<PluginInfoUi>,
    // ── Extended UiState (additive, per CONTRACT.md) ──
    bookmarks: Vec<bookmarks::Bookmark>,
    split: bool,
    /// flags.onboarded falsy → frontend shows the preset-picker overlay.
    first_run: bool,
    /// New-tab page config (custom URL + built-in page customization).
    newtab: config::NewTabConfig,
    /// User's custom design-token overrides ([theme.tokens] in umbra.toml).
    #[serde(rename = "themeTokens")]
    theme_tokens: std::collections::BTreeMap<String, String>,
    /// Configured User-Agent override (empty = built-in default). Mirrored from
    /// config.user_agent so the settings UI can bind/edit it directly.
    #[serde(rename = "userAgent")]
    user_agent: String,
    /// Effective token values (palette + installed theme + overrides) for
    /// placeholders in the settings GUI.
    #[serde(rename = "resolvedTokens")]
    resolved_tokens: std::collections::BTreeMap<String, String>,
    /// True when compiled in debug mode - frontend shows a dismissable debug banner.
    is_debug: bool,
}

#[derive(Serialize, Clone)]
struct PluginInfoUi {
    name: String,
    status: String,
    errors: Vec<String>,
}

fn ui_state(app: &AppHandle) -> UiState {
    let state = app.state::<AppState>();
    let cfg = state.config.read().unwrap();
    let mut resolved = cfg.clone();
    if !state.safe_mode {
        if let Some(tokens) = state.store.load_theme_tokens(&cfg.theme.name) {
            for (k, v) in tokens {
                resolved.theme.tokens.insert(k, v);
            }
        }
    }
    let out = UiState {
        tabs: tabs::tab_infos(&state),
        layout: cfg.layout.preset.clone(),
        mode: cfg.theme.mode.clone(),
        theme_preset: cfg.theme.preset.clone(),
        safe_mode: state.safe_mode,
        bookmarks_bar: cfg.layout.bookmarks_bar,
        status_bar: cfg.layout.status_bar,
        sidebar_width: cfg.layout.sidebar_width,
        keybindings: cfg.keybindings.clone(),
        flags: cfg.flags.clone(),
        actions: config::KNOWN_ACTIONS.to_vec(),
        layouts: config::LAYOUT_PRESETS.to_vec(),
        presets: config::THEME_PRESETS.to_vec(),
        slots: slots::SLOTS.to_vec(),
        mounted: state.slots.lock().unwrap().mounted.clone(),
        plugins: state
            .plugins
            .lock()
            .unwrap()
            .loaded
            .iter()
            .map(|p| PluginInfoUi {
                name: p.manifest.name.clone(),
                status: p.status.clone(),
                errors: p.errors.clone(),
            })
            .collect(),
        bookmarks: state.bookmarks.lock().unwrap().items.clone(),
        split: state.tabs.lock().unwrap().split.is_some(),
        // Live updates report the real persisted flag so changing settings/themes
        // mid-session does not reopen onboarding. The launch command
        // (`get_ui_state`) still forces it in debug to exercise the flow.
        first_run: !cfg.flags.get("onboarded").copied().unwrap_or(false),
        newtab: cfg.newtab.clone(),
        theme_tokens: cfg.theme.tokens.clone(),
        user_agent: cfg.user_agent.clone(),
        // Effective values (built-in palette + installed theme + overrides)
        // so the settings GUI can placeholder what an unset token resolves to.
        resolved_tokens: resolved.theme.resolved_tokens(),
        is_debug: cfg!(debug_assertions),
    };
    out
}

/// Persist a mutated config and push it live (settings GUI writes the TOML).
fn persist_and_apply(app: &AppHandle, mutate: impl FnOnce(&mut config::Config)) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        let mut cfg = state.config.write().unwrap();
        mutate(&mut cfg);
        cfg.validate()?;
        state.store.save(&cfg);
    }
    watcher::snapshot_current(app);
    watcher::apply_theme(app, &state);
    tabs::emit_tabs(app, &state);
    // Emit full UI state so frontend stays in sync (theme_preset, layout, etc.)
    let ui = ui_state(app);
    app.emit("umbra://ui_state", serde_json::to_value(&ui).unwrap_or_default()).ok();
    Ok(())
}

#[tauri::command]
fn get_ui_state(app: AppHandle) -> Result<UiState, String> {
    let s = ui_state(&app);
    // In debug builds, force onboarding on every launch to exercise the flow
    // without touching real user data - but live state updates above use the
    // real flag so it does not reopen while the app is already running.
    #[cfg(debug_assertions)]
    let s = {
        let mut s = s;
        s.first_run = true;
        s
    };
    Ok(s)
}

#[tauri::command]
fn create_tab(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    tabs::create_tab(&app, &state, None);
    tabs::emit_tabs(&app, &state);
    Ok(())
}

#[tauri::command]
fn close_tab(app: AppHandle, id: u32) -> Result<(), String> {
    let state = app.state::<AppState>();
    tabs::close_tab(&app, &state, id)
}

#[tauri::command]
fn activate_tab(app: AppHandle, id: u32) -> Result<(), String> {
    let state = app.state::<AppState>();
    tabs::activate_tab(&app, &state, id)
}

#[tauri::command]
fn navigate(app: AppHandle, url: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    tabs::navigate(&app, &state, &url)
}

#[tauri::command]
fn go_back(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    tabs::go_back(&app, &state)
}

#[tauri::command]
fn go_forward(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    tabs::go_forward(&app, &state)
}

#[tauri::command]
fn reload_page(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    tabs::reload(&app, &state)
}

#[tauri::command]
fn update_content_bounds(app: AppHandle, x: f64, y: f64, w: f64, h: f64) {
    let state = app.state::<AppState>();
    tabs::set_content_bounds(&app, &state, x, y, w, h);
}

#[tauri::command]
 fn set_webviews_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    tabs::set_webviews_visible(&app, &state, visible);
    Ok(())
}

/// Toggle the chrome "capture all" mode for the hamburger dropdown / in-content
/// chrome popups so their (content-overlapping) hit area stays clickable.
#[tauri::command]
fn set_menu_open(app: AppHandle, open: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    tabs::set_menu_open(&app, &state, open);
    Ok(())
}

// ── Split view ───────────────────────────────────────────────────────────────

/// Toggle split view; returns the new on/off state.
#[tauri::command]
fn toggle_split(app: AppHandle) -> Result<bool, String> {
    let state = app.state::<AppState>();
    tabs::toggle_split(&app, &state)
}

// ── Bookmarks & history ──────────────────────────────────────────────────────

#[tauri::command]
fn list_bookmarks(app: AppHandle) -> Result<Vec<bookmarks::Bookmark>, String> {
    let state = app.state::<AppState>();
    let items = state.bookmarks.lock().unwrap().items.clone();
    Ok(items)
}

#[tauri::command]
fn add_bookmark(
    app: AppHandle,
    title: Option<String>,
    url: String,
) -> Result<Vec<bookmarks::Bookmark>, String> {
    let state = app.state::<AppState>();
    let bm = {
        let mut bms = state.bookmarks.lock().unwrap();
        bms.add(title.unwrap_or_else(|| url.clone()), url)
    };
    state.bookmarks.lock().unwrap().save(&state.store);
    emit_bookmarks(&app, &state, Some(format!("Bookmarked “{}”", bm.title)));
    let items = state.bookmarks.lock().unwrap().items.clone();
    Ok(items)
}

#[tauri::command]
fn remove_bookmark(app: AppHandle, id: u32) -> Result<Vec<bookmarks::Bookmark>, String> {
    let state = app.state::<AppState>();
    {
        let mut store = state.bookmarks.lock().unwrap();
        store.items.retain(|b| b.id != id);
        store.save(&state.store);
    }
    emit_bookmarks(&app, &state, None);
    let items = state.bookmarks.lock().unwrap().items.clone();
    Ok(items)
}

/// Bookmark the active tab (title + current URL). Used by run_action
/// "bookmark_page" (Ctrl+D), confirmed via a toast.
pub fn bookmark_page(app: &AppHandle, state: &tauri::State<AppState>) -> Result<(), String> {
    let (title, url) = {
        let mgr = state.tabs.lock().unwrap();
        let id = mgr.active.ok_or("no active tab")?;
        let tab = mgr.get(id).ok_or("no active tab")?;
        (tab.title.clone(), tab.history[tab.hist_idx].clone())
    };
    let bm = {
        let mut bms = state.bookmarks.lock().unwrap();
        bms.add(title, url)
    };
    state.bookmarks.lock().unwrap().save(&state.store);
    emit_bookmarks(app, state, Some(format!("Bookmarked “{}”", bm.title)));
    Ok(())
}

/// Push the bookmark list to the chrome UI plus an optional toast.
fn emit_bookmarks(app: &AppHandle, state: &tauri::State<AppState>, toast: Option<String>) {
    let items = state.bookmarks.lock().unwrap().items.clone();
    app.emit("umbra://state", serde_json::json!({ "bookmarks": items })).ok();
    if let Some(message) = toast {
        app.emit("umbra://toast", serde_json::json!({"message": message, "kind": "ok"})).ok();
    }
}

#[tauri::command]
fn list_history(
    app: AppHandle,
    query: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<history::HistoryEntry>, String> {
    let state = app.state::<AppState>();
    let limit = limit.unwrap_or(200).max(1) as usize;
    match query {
        Some(q) if !q.trim().is_empty() => {
            let hist = state.history.lock().unwrap();
            Ok(hist
                .entries
                .iter()
                .filter(|e| e.title.to_lowercase().contains(&q.to_lowercase()))
                .take(limit)
                .cloned()
                .collect())
        }
        _ => {
            let hist = state.history.lock().unwrap();
            Ok(hist.entries.iter().take(limit).cloned().collect())
        }
    }
}

#[tauri::command]
fn clear_history(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.history.lock().unwrap().clear();
    state.history.lock().unwrap().save(&state.store);
    Ok(())
}

// ── Packages / gallery (.umbrapkg pipeline) ─────────────────────────────────

/// Stage a package from an https URL / GitHub owner-repo / local path.
/// Stages only - nothing applied until apply_pending_package.
#[tauri::command]
fn install_package(app: AppHandle, source: String) -> Result<pkg::PkgDiff, String> {
    pkg::stage_from_source(&app, &source)
}

#[tauri::command]
fn apply_pending_package(app: AppHandle) -> Result<String, String> {
    pkg::apply_pending(&app)
}

#[tauri::command]
fn cancel_pending_package(app: AppHandle) -> Result<(), String> {
    pkg::cancel_pending(&app);
    Ok(())
}

#[tauri::command]
fn gallery_list() -> Result<Vec<pkg::GalleryItem>, String> {
    pkg::gallery_list()
}

#[tauri::command]
fn gallery_install(app: AppHandle, repo: String) -> Result<pkg::PkgDiff, String> {
    // Resolves the latest-release .umbrapkg asset for owner/name via the
    // GitHub API, then runs the same staged pipeline as install_package.
    pkg::stage_from_source(&app, &repo)
}

// ── Browser import (pass-2) ──────────────────────────────────────────────────

#[tauri::command]
fn import_detect() -> Result<Vec<import::ImportSource>, String> {
    Ok(import::detect())
}

#[tauri::command]
fn import_browser(
    app: AppHandle,
    browser: String,
    profile: String,
    bookmarks: bool,
    history: bool,
) -> Result<import::ImportResult, String> {
    import::import_browser(&app, &browser, &profile, bookmarks, history)
}

// ── Config/bookmark gist sync (pass-2) ───────────────────────────────────────

#[tauri::command]
fn sync_status(app: AppHandle) -> Result<sync::SyncStatus, String> {
    Ok(sync::status(&app))
}

/// token/gist_id empty string clears the field.
#[tauri::command]
fn sync_configure(
    app: AppHandle,
    token: Option<String>,
    gist_id: Option<String>,
) -> Result<sync::SyncStatus, String> {
    sync::configure(&app, token, gist_id)
}

#[tauri::command]
fn sync_push(app: AppHandle) -> Result<String, String> {
    sync::push(&app)
}

#[tauri::command]
fn sync_pull(app: AppHandle) -> Result<String, String> {
    sync::pull(&app)
}

// ── Picture-in-picture (pass-2) ──────────────────────────────────────────────

/// Toggle the PiP window; returns the new on/off state.
#[tauri::command]
fn pip_toggle(app: AppHandle) -> Result<bool, String> {
    pip::toggle(&app)
}

// ── Config-driven settings (all persisted to umbra.toml) ────────────────────

#[tauri::command]
fn set_layout(app: AppHandle, preset: String) -> Result<(), String> {
    if !config::LAYOUT_PRESETS.contains(&preset.as_str()) {
        return Err(format!("unknown preset '{preset}'"));
    }
    persist_and_apply(&app, |cfg| cfg.layout.preset = preset)
}

#[tauri::command]
fn set_mode(app: AppHandle, mode: String) -> Result<(), String> {
    if !config::MODES.contains(&mode.as_str()) {
        return Err(format!("mode must be dark or light, got '{mode}'"));
    }
    persist_and_apply(&app, |cfg| cfg.theme.mode = mode)
}

#[tauri::command]
fn set_theme_preset(app: AppHandle, preset: String) -> Result<(), String> {
    if !config::THEME_PRESETS.contains(&preset.as_str()) && preset != "custom" {
        return Err(format!("unknown theme preset '{preset}'"));
    }
    persist_and_apply(&app, |cfg| cfg.theme.preset = preset)
}

/// Set a single design token (custom colors etc.). Empty value removes the override.
#[tauri::command]
fn set_token(app: AppHandle, key: String, value: String) -> Result<(), String> {
    persist_and_apply(&app, |cfg| {
        if value.trim().is_empty() {
            cfg.theme.tokens.remove(&key);
        } else {
            cfg.theme.tokens.insert(key, value);
        }
    })
}

#[tauri::command]
fn reset_tokens(app: AppHandle) -> Result<(), String> {
    persist_and_apply(&app, |cfg| cfg.theme.tokens.clear())
}

/// Set the User-Agent override sent to all sites. Empty string restores the
/// built-in default (desktop Firefox clone). Env UMBRA_USER_AGENT still wins when set.
#[tauri::command]
fn set_user_agent(app: AppHandle, value: String) -> Result<(), String> {
    persist_and_apply(&app, |cfg| {
        cfg.user_agent = value;
    })
}

#[tauri::command]
fn set_keybinding(app: AppHandle, combo: String, action: String) -> Result<(), String> {
    persist_and_apply(&app, |cfg| {
        if action.is_empty() {
            cfg.keybindings.remove(&combo);
        } else {
            cfg.keybindings.insert(combo, action);
        }
    })
}

#[tauri::command]
fn set_flag(app: AppHandle, key: String, value: bool) -> Result<(), String> {
    persist_and_apply(&app, |cfg| {
        cfg.flags.insert(key, value);
    })
}

/// Configure the new-tab page. Empty `url` = built-in page; the other fields
/// customize the built-in page itself.
#[tauri::command]
fn set_newtab(
    app: AppHandle,
    url: Option<String>,
    background: Option<String>,
    greeting: Option<String>,
    show_clock: Option<bool>,
) -> Result<(), String> {
    persist_and_apply(&app, |cfg| {
        if let Some(v) = url {
            cfg.newtab.url = v.trim().to_string();
        }
        if let Some(v) = background {
            cfg.newtab.background = v.trim().to_string();
        }
        if let Some(v) = greeting {
            cfg.newtab.greeting = v;
        }
        if let Some(v) = show_clock {
            cfg.newtab.show_clock = v;
        }
    })
}

// ── Config-file editors (settings GUI → every TOML in the config dir) ──────

#[tauri::command]
fn config_list_files(app: AppHandle) -> Vec<config::ConfigFileInfo> {
    app.state::<AppState>().store.list_toml_files()
}

#[tauri::command]
fn config_read_file(app: AppHandle, path: String) -> Result<String, String> {
    let store = &app.state::<AppState>().store;
    let p = store.resolve_toml_path(&path)?;
    fs::read_to_string(&p).map_err(|e| format!("cannot read {}: {e}", p.display()))
}

/// Save an edited TOML file. umbra.toml (and theme/layout/plugin files) are
/// validated then hot-applied; bookmarks/history are reloaded into memory.
#[tauri::command]
fn config_write_file(app: AppHandle, path: String, content: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let p = state.store.resolve_toml_path(&path)?;
    // Validate before writing so a typo can never take the browser down.
    match path.as_str() {
        "umbra.toml" => {
            config::Config::parse_str(&content)?;
        }
        _ => {
            toml::from_str::<toml::Value>(&content)
                .map_err(|e| format!("TOML parse error: {e}"))?;
        }
    }
    std::fs::write(&p, content).map_err(|e| format!("cannot write {}: {e}", p.display()))?;
    // Apply live where it's safe to do so.
    if path == "umbra.toml"
        || path.starts_with("themes/")
        || path.starts_with("layouts/")
        || path.starts_with("plugins/")
    {
        watcher::reload_everything(&app);
    } else if path == "bookmarks.toml" {
        let loaded = crate::bookmarks::Bookmarks::load(&state.store);
        *state.bookmarks.lock().unwrap() = loaded;
        emit_bookmarks(&app, &state, Some("Bookmarks reloaded".into()));
    } else if path == "history.toml" {
        let loaded = crate::history::History::load(&state.store);
        *state.history.lock().unwrap() = loaded;
    }
    Ok(())
}

#[tauri::command]
fn reload_config(app: AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    match state.store.load_or_rollback() {
        Ok((cfg, rollback)) => {
            *state.config.write().unwrap() = cfg;
            watcher::snapshot_current(&app);
            watcher::apply_theme(&app, &state);
            tabs::emit_tabs(&app, &state);
            Ok(rollback.unwrap_or_else(|| "ok".into()))
        }
        Err(e) => Err(e),
    }
}

/// Emit a dedicated UI-panel event. CONTRACT: `umbra://ui` carries
/// `{open: "settings"|"console"|null}` (+ optional extras like `modal`);
/// the Svelte chrome listens and opens/closes the matching panel.
fn emit_ui(app: &AppHandle, open: Option<&str>, modal: Option<bool>) {
    let mut payload = serde_json::json!({"open": open});
    if let Some(m) = modal {
        payload["modal"] = serde_json::json!(m);
    }
    app.emit("umbra://ui", payload).ok();
}

/// Execute a named action (palette entries, remappable keys, control socket).
#[tauri::command]
fn run_action(app: AppHandle, action: String, arg: Option<String>) -> Result<(), String> {
    let state = app.state::<AppState>();
    match action.as_str() {
        "new_tab" => {
            tabs::create_tab(&app, &state, arg);
            tabs::emit_tabs(&app, &state);
            Ok(())
        }
        "close_tab" => {
            let id = state.tabs.lock().unwrap().active_id();
            if let Some(id) = id {
                tabs::close_tab(&app, &state, id)
            } else {
                Ok(())
            }
        }
        "next_tab" => {
            tabs::next_tab(&app, &state, 1);
            Ok(())
        }
        "prev_tab" => {
            tabs::next_tab(&app, &state, -1);
            Ok(())
        }
        "back" => tabs::go_back(&app, &state),
        "forward" => tabs::go_forward(&app, &state),
        "reload" => tabs::reload(&app, &state),
        "focus_url" => {
            if let Some(wv) = app.get_webview("chrome") {
                // eval is a WebKitGTK call - dispatch to the main thread.
                crate::maindispatch::on_main(&app, move || {
                    wv.eval("window.__umbraFocusUrl && window.__umbraFocusUrl();").ok();
                });
            }
            Ok(())
        }
        "reload_config" => reload_config(app).map(|_| ()),
        "toggle_mode" => {
            let cur = state.config.read().unwrap().theme.mode.clone();
            let next = if cur == "dark" { "light" } else { "dark" };
            set_mode(app, next.to_string())
        }
        "cycle_layout" => {
            let all = config::LAYOUT_PRESETS;
            let cur = state.config.read().unwrap().layout.preset.clone();
            let idx = all.iter().position(|p| *p == cur).map(|i| i + 1).unwrap_or(0) % all.len();
            set_layout(app, all[idx].to_string())
        }
        "bookmark_page" => bookmark_page(&app, &state),
        "toggle_split" => toggle_split(app).map(|_| ()),
        "open_settings" => {
            emit_ui(&app, Some("settings"), None);
            Ok(())
        }
        "open_console" => {
            emit_ui(&app, Some("console"), None);
            Ok(())
        }
        "toggle_modal" => {
            // Modal/vim-style command mode flag; frontend reacts to umbra://ui.
            let now_on = {
                let mut cfg = state.config.write().unwrap();
                let next = !*cfg.flags.get("modal_mode").unwrap_or(&false);
                cfg.flags.insert("modal_mode".into(), next);
                next
            };
            persist_and_apply(&app, |_| {})?;
            emit_ui(&app, None, Some(now_on));
            Ok(())
        }
        "pip" => pip::toggle(&app).map(|_| ()),
        "palette" => {
            // Command palette lives in the chrome; it listens for umbra://ui.
            emit_ui(&app, Some("palette"), None);
            Ok(())
        }
        "toggle_fullscreen" => {
            let win = app.get_window("main").ok_or("main window missing")?;
            let fs = win.is_fullscreen().map_err(|e| e.to_string())?;
            win.set_fullscreen(!fs).map_err(|e| e.to_string())
        }
        other => Err(format!("unknown action '{other}'")),
    }
}

/// Normalize a GDK key event into the frontend's combo strings (see
/// `comboOf()` in ui/src/lib/keybinds.ts) so one keybinding table drives both
/// layers: `Ctrl+Shift+D`, `Alt+Left`, `F11`, `Ctrl+,`, `Ctrl+\`, …
#[cfg(target_os = "linux")]
fn combo_from_event(ev: &gtk::gdk::EventKey) -> Option<String> {
    use gtk::gdk;
    let mods = ev.state();
    let mut c = String::new();
    // Same modifier order as comboOf(): Ctrl, Alt, Meta, Shift.
    if mods.contains(gdk::ModifierType::CONTROL_MASK) {
        c.push_str("Ctrl+");
    }
    if mods.contains(gdk::ModifierType::MOD1_MASK) {
        c.push_str("Alt+");
    }
    if mods.contains(gdk::ModifierType::SUPER_MASK) {
        c.push_str("Meta+");
    }
    let name = ev.keyval().name()?.to_string();
    let k = match name.as_str() {
        "space" => "Space".to_string(),
        "Escape" => "Esc".to_string(),
        "Return" | "KP_Enter" => "Enter".to_string(),
        // Shift+Tab arrives as its own keyval; the frontend sees "Tab".
        "ISO_Left_Tab" => "Tab".to_string(),
        "backslash" => "\\".to_string(),
        "comma" => ",".to_string(),
        "period" => ".".to_string(),
        "slash" => "/".to_string(),
        "minus" => "-".to_string(),
        "equal" => "=".to_string(),
        "bracketleft" => "[".to_string(),
        "bracketright" => "]".to_string(),
        "grave" => "`".to_string(),
        "semicolon" => ";".to_string(),
        "apostrophe" => "'".to_string(),
        "Page_Up" => "PageUp".to_string(),
        "Page_Down" => "PageDown".to_string(),
        "Print" => "PrintScreen".to_string(),
        // Named keys (Tab, F5, Left, …) pass through unchanged, matching what
        // KeyboardEvent.key gives the frontend for the same physical key.
        _ if name.len() > 1 => name,
        // Single characters are upper-cased the way comboOf() does.
        _ => name.to_uppercase(),
    };
    // Shift matters only for keys whose identity it doesn't already encode -
    // the exact mirror of comboOf()'s rule, so one table serves both layers
    // ("Ctrl+Shift+D" stays distinct from "Ctrl+D", while "\" or "," match
    // their base-key bindings even when the layout types them with Shift).
    if mods.contains(gdk::ModifierType::SHIFT_MASK)
        && (k.len() > 1 || k.chars().any(|ch| ch.is_ascii_alphanumeric()))
    {
        c.push_str("Shift+");
    }
    Some(c + &k)
}

pub fn run() {
    let safe_mode = std::env::args().any(|a| a == "--safe");

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_ui_state,
            create_tab,
            close_tab,
            activate_tab,
            navigate,
            go_back,
            go_forward,
            reload_page,
            update_content_bounds,
            set_webviews_visible,
            set_menu_open,
            toggle_split,
            list_bookmarks,
            add_bookmark,
            remove_bookmark,
            list_history,
            clear_history,
            install_package,
            apply_pending_package,
            cancel_pending_package,
            gallery_list,
            gallery_install,
            import_detect,
            import_browser,
            sync_status,
            sync_configure,
            sync_push,
            sync_pull,
            pip_toggle,
            set_layout,
            set_mode,
            set_theme_preset,
            set_token,
            reset_tokens,
            set_user_agent,
            set_keybinding,
            set_flag,
            set_newtab,
            reload_config,
            config_list_files,
            config_read_file,
            config_write_file,
            run_action,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // Config store first so the saved session can size the window.
            let store = config::ConfigStore::new();
            store.ensure_default();
            let boot_session = if safe_mode { None } else { session::load(&store) };
            let (w0, h0) = boot_session
                .as_ref()
                .map(|s| (s.width, s.height))
                .unwrap_or((1280.0, 800.0));

            // Transparency crashes some Wayland compositors at the protocol
            // level (error 71) before any webview renders. Keep DMABUF enabled
            // and just opt out of ARGB visuals on Wayland; glassmorphism still
            // works via backdrop-filter inside the chrome webview.
            // Alpha on: the chrome webview is stacked ABOVE the tab webviews so
            // its content area can be see-through (the opaque tab page fills it),
            // letting the settings/hamburger UI paint over the page. The tab
            // webview always provides opaque content, so under sway/XWayland the
            // window is never the "invisible ARGB" case (it only happens when
            // nothing in the window has painted).
            let builder = WindowBuilder::new(&handle, "main")
                .title("Umbra")
                .inner_size(w0.max(640.0), h0.max(400.0))
                .min_inner_size(640.0, 400.0)
                .decorations(false)
                .transparent(true);
            let window = builder.build()?;
            let scale = window.scale_factor().unwrap_or(1.0);
            // Some WMs drop undecorated windows at the edge of a multi-monitor
            // span where the user never sees them - place explicitly on the
            // primary monitor and make sure the session/default size stuck.
            {
                use tauri::{PhysicalPosition, PhysicalSize};
                let mut w = window.inner_size().unwrap_or_default();
                if w.width < 640 || w.height < 400 {
                    // Floor at the minimum usable size, never restore garbage.
                    let want = PhysicalSize::new(
                        (w0.max(640.0) * scale) as u32,
                        (h0.max(400.0) * scale) as u32,
                    );
                    window.set_size(want).ok();
                    w = want;
                }
                if let Ok(Some(primary)) = window.current_monitor() {
                    let mpos = primary.position();
                    let msize = primary.size();
                    let x = mpos.x + ((msize.width.saturating_sub(w.width)) / 2) as i32;
                    let y = mpos.y + ((msize.height.saturating_sub(w.height)) / 2) as i32;
                    window.set_position(PhysicalPosition::new(x, y)).ok();
                }
            }
            let physical = window.inner_size().unwrap_or_default();

            // Chrome webview - the only UI Umbra ships hardcoded; everything it
            // renders is slot-mounted and token-driven.
            // Must be transparent so the opaque tab webviews underneath show
            // through the content area (which has body: transparent). Without
            // this, WebKitGTK paints an opaque white background over every
            // page - the "white screen but audio still plays" bug.
            let chrome = WebviewBuilder::new("chrome", WebviewUrl::App("index.html".into()))
                .transparent(true);
            window.add_child(
                chrome,
                tauri::PhysicalPosition::new(0, 0),
                tauri::PhysicalSize::new(physical.width.max(1), physical.height.max(1)),
            )?;
            // On Linux, add_child() drops chrome and every future tab webview
            // into the window's gtk::Box, which lays siblings out
            // sequentially instead of overlaying them - that's the cause of
            // the empty gap / status bar showing above tab content. Move
            // chrome into a shared gtk::Fixed so it can be a full-window
            // base layer with tab webviews absolutely positioned on top.
            // See linux_overlay.rs for details.
            #[cfg(target_os = "linux")]
            {
                if let Some(chrome_wv) = handle.get_webview("chrome") {
                    let (_tabs_fixed, chrome_fixed) = linux_overlay::get_or_create_fixeds(&window)?;
                    linux_overlay::embed(
                        &chrome_fixed,
                        &chrome_wv,
                        0,
                        0,
                        physical.width.max(1) as i32,
                        physical.height.max(1) as i32,
                    )?;
                }
            }

            let handle_for_resize = handle.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::Resized(physical_size) = event {
                    if let Some(chrome_wv) = handle_for_resize.get_webview("chrome") {
                        #[cfg(not(target_os = "linux"))]
                        {
                            chrome_wv
                                .set_bounds(tauri::Rect {
                                    position: tauri::PhysicalPosition::new(0, 0).into(),
                                    size: tauri::PhysicalSize::new(
                                        physical_size.width.max(1),
                                        physical_size.height.max(1),
                                    )
                                    .into(),
                                })
                                .ok();
                        }
                        #[cfg(target_os = "linux")]
                        {
                            // Chrome lives in the shared overlay gtk::Fixed now
                            // (see setup, above) - it must be resized manually
                            // here, same as any other absolutely-positioned
                            // widget; gtk::Fixed does not auto-fill like Box did.
                            if let Some(window) = handle_for_resize.get_window("main") {
                                if let Ok((tabs_fixed, chrome_fixed)) =
                                    crate::linux_overlay::get_or_create_fixeds(&window)
                                {
                                    let (w, h) = (
                                        physical_size.width.max(1) as i32,
                                        physical_size.height.max(1) as i32,
                                    );
                                    // Keep both overlay layers sized to the window
                                    // so the (absolutely-positioned) tab webviews
                                    // inside them stay aligned.
                                    tabs_fixed.set_size_request(w, h);
                                    chrome_fixed.set_size_request(w, h);
                                    let _ = crate::linux_overlay::reposition(
                                        &chrome_fixed,
                                        &chrome_wv,
                                        0,
                                        0,
                                        w,
                                        h,
                                    );
                                }
                            }
                        }
                    }
                }
            });

            let (cfg, boot_error) = if safe_mode {
                (config::Config::default(), None)
            } else {
                match store.load_or_rollback() {
                    Ok((c, e)) => (c, e),
                    Err(e) => {
                        eprintln!("[umbra] config load failed, using defaults: {e}");
                        (config::Config::default(), Some(e))
                    }
                }
            };

            debug_log!(
                "boot safe_mode={safe_mode} theme={} layout={} boot_error={:?}",
                cfg.theme.preset,
                cfg.layout.preset,
                boot_error
            );

            let mut registry = slots::SlotRegistry::with_defaults();
            let mut plugin_host = plugins::PluginHost::default();
            let plugin_warnings = if safe_mode {
                vec!["--safe: plugins and user config disabled".to_string()]
            } else {
                plugin_host.load_all(store.plugins_dir(), &cfg.plugins, &mut registry)
            };
            debug_log!(
                "plugins loaded: {} (warnings: {}) slots: {}",
                plugin_warnings.is_empty(),
                plugin_warnings.len(),
                registry.mounted.len()
            );

            let bookmarks_store = bookmarks::Bookmarks::load(&store);
            let history_store = history::History::load(&store);

            app.manage(AppState {
                config: std::sync::RwLock::new(cfg.clone()),
                tabs: std::sync::Mutex::new(tabs::TabManager::with_content((
                    0.0,
                    52.0,
                    physical.width as f64,
                    (physical.height as f64 - 52.0).max(1.0),
                ))),
                slots: std::sync::Mutex::new(registry),
                plugins: std::sync::Mutex::new(plugin_host),
                store,
                safe_mode,
                bookmarks: std::sync::Mutex::new(bookmarks_store),
                history: std::sync::Mutex::new(history_store),
                pending_pkg: std::sync::Mutex::new(None),
                pip_open: std::sync::atomic::AtomicBool::new(false),
                sync_error: std::sync::Mutex::new(None),
            });

            // Boot a starter tab, or restore the previous session (--safe ignores it).
            {
                let state = handle.state::<AppState>();
                let restored = if safe_mode {
                    false
                } else {
                    session::restore(&handle)
                };
                if !restored {
                    tabs::create_tab(&handle, &state, None);
                }
            }

            // Push initial chrome state once the webview is up.
            let h2 = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(600));
                let state = h2.state::<AppState>();
                watcher::apply_theme(&h2, &state);
                tabs::emit_tabs(&h2, &state);
                for w in plugin_warnings {
                    h2.emit("umbra://console", serde_json::json!({"level":"warn","message":w}))
                        .ok();
                }
                if let Some(e) = boot_error {
                    h2.emit(
                        "umbra://toast",
                        serde_json::json!({"message": format!("Boot: {e}"), "kind": "error"}),
                    )
                    .ok();
                }
                if safe_mode {
                    h2.emit(
                        "umbra://toast",
                        serde_json::json!({"message": "Safe mode: config and plugins disabled", "kind": "info"}),
                    )
                    .ok();
                } else if cfg!(debug_assertions) {
                    h2.emit(
                        "umbra://toast",
                        serde_json::json!({"message": "Running in debug mode. Onboarding will show on every launch.", "kind": "info"}),
                    )
                    .ok();
                }
            });

            // GTK-level shortcut fallback for page tabs. While a real web page
            // has focus, WebKitGTK consumes most key events for the DOM, so the
            // chrome's window keydown never fires and bound shortcuts (Ctrl+T,
            // Ctrl+K, …) go dead - that's the "keybinds only work on the
            // new-tab page" bug (on the new-tab page the focus sits in the
            // chrome webview, which handles them itself). GTK still bubbles
            // unhandled key presses from the focused widget up to the toplevel
            // (the same mechanism Epiphany relies on with WebKitGTK), so we
            // resolve bound combos here and dispatch run_action. Keys focused
            // inside the chrome webview are skipped so its own key handling
            // stays authoritative - no double-firing.
            #[cfg(target_os = "linux")]
            if let Some(win) = handle.get_window("main") {
                if let Ok(gtk_win) = win.gtk_window() {
                    let hk = handle.clone();
                    gtk_win.connect_key_press_event(move |w, ev| {
                        // Focused inside the chrome webview? Let the chrome DOM
                        // handle this key (it owns its own keydown path).
                        let mut cur = w.focused_widget();
                        let mut in_chrome = false;
                        while let Some(fw) = cur {
                            if fw.widget_name() == "umbra-chrome-fixed" {
                                in_chrome = true;
                                break;
                            }
                            cur = fw.parent();
                        }
                        if in_chrome {
                            return gtk::glib::Propagation::Proceed;
                        }
                        let Some(combo) = combo_from_event(ev) else {
                            return gtk::glib::Propagation::Proceed;
                        };
                        let state = hk.state::<AppState>();
                        // The hamburger menu owns its keys while open (mirrors
                        // the chrome's rule of ignoring shortcuts then).
                        if state.tabs.lock().unwrap().menu_open {
                            return gtk::glib::Propagation::Proceed;
                        }
                        // User keybindings override the built-in defaults.
                        let mut kb = config::default_keybindings();
                        for (k, v) in state.config.read().unwrap().keybindings.iter() {
                            kb.insert(k.clone(), v.clone());
                        }
                        let Some(action) = kb.get(&combo).cloned() else {
                            return gtk::glib::Propagation::Proceed;
                        };
                        debug_log!("gtk key {combo} -> {action} (page tab focused)");
                        match run_action(hk.clone(), action, None) {
                            Ok(()) => gtk::glib::Propagation::Stop,
                            Err(e) => {
                                debug_log!("gtk key {combo} failed: {e}");
                                gtk::glib::Propagation::Proceed
                            }
                        }
                    });
                }
            }

            // Hot reload + control socket.
            watcher::spawn(handle.clone())?;
            ipc::spawn(handle)?;

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building umbra")
        .run(|app, event| {
            // Session persistence right before exit (save-on-change already
            // covers every tab mutation; this catches the final state).
            if matches!(event, tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }) {
                session::save(app);
            }
        });
}
