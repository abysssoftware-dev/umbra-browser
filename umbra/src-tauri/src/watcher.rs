use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use crate::debug_log;

/// Filesystem watcher on the config directory: applies config/theme/plugin
/// changes live without restart. Failed reloads roll back with a toast.
pub fn spawn(app: AppHandle) -> Result<(), String> {
    let watch_dir: PathBuf = {
        let state = app.state::<crate::state::AppState>();
        state.store.config_dir()
    };
    std::thread::spawn(move || watcher_loop(app, watch_dir));
    Ok(())
}

fn watcher_loop(app: AppHandle, watch_dir: PathBuf) {
    let (tx, rx) = channel::<Event>();
    let mut _watcher = make_watcher(&watch_dir, &tx);
    let start_time = Instant::now();
    let mut last = Instant::now();
    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(ev) => {
                // Ignore events during the first 2 seconds of boot so startup file creation doesn't fire spurious reloads
                if start_time.elapsed() < Duration::from_millis(2000) {
                    continue;
                }
                let relevant_kind = matches!(
                    ev.kind,
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                );
                // Check if any path is actually a configuration / theme / plugin file (ignore session.toml, history.toml, bookmarks.toml, log files)
                let is_config_file = ev.paths.iter().any(|p| {
                    let s = p.to_string_lossy();
                    s.ends_with("umbra.toml")
                        || s.contains("/themes/")
                        || s.contains("/plugins/")
                        || s.contains("/layouts/")
                });

                if relevant_kind && is_config_file && last.elapsed() > Duration::from_millis(300) {
                    last = Instant::now();
                    std::thread::sleep(Duration::from_millis(250));
                    while rx.try_recv().is_ok() {}
                    reload_everything(&app);
                    // re-watch in case directories were recreated
                    _watcher = make_watcher(&watch_dir, &tx);
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
    }
}

fn make_watcher(
    watch_dir: &PathBuf,
    tx: &std::sync::mpsc::Sender<Event>,
) -> Option<RecommendedWatcher> {
    let tx = tx.clone();
    let mut w: Option<RecommendedWatcher> =
        notify::recommended_watcher(move |res: Result<Event, _>| {
            if let Ok(ev) = res {
                tx.send(ev).ok();
            }
        })
        .ok();
    if let Some(watcher) = w.as_mut() {
        watcher.watch(watch_dir, RecursiveMode::Recursive).ok();
    }
    w
}

fn resolved_config(state: &tauri::State<crate::state::AppState>) -> crate::config::Config {
    let store = &state.store;
    let mut cfg = state.config.read().unwrap().clone();
    if !state.safe_mode {
        // Installed theme overrides first… (user config, skipped in --safe)
        if let Some(tokens) = store.load_theme_tokens(&cfg.theme.name) {
            for (k, v) in tokens {
                cfg.theme.tokens.insert(k, v);
            }
        }
    }
    // …then full resolution against the built-in palette so preset/mode swaps
    // actually change the emitted CSS (the themes bug). Always resolved - even
    // in --safe, which only disables *user* config: skipping it here emitted an
    // empty `:root {}`, making every theme look unchanged and leaving the top
    // bar on its translucent fallback background.
    cfg.theme.tokens = cfg.theme.resolved_tokens();
    cfg
}

pub fn snapshot_current(app: &AppHandle) {
    let state = app.state::<crate::state::AppState>();
    let cfg = state.config.read().unwrap().clone();
    state.store.snapshot_good(&cfg);
}

pub fn reload_everything(app: &AppHandle) {
    let state = app.state::<crate::state::AppState>();
    if state.safe_mode {
        return; // --safe boots with all plugins/config disabled
    }
    let store = &state.store;

    match store.load_or_rollback() {
        Ok((cfg, rollback_error)) => {
            store.snapshot_good(&cfg);

            let warnings = {
                let mut registry = state.slots.lock().unwrap();
                *registry = crate::slots::SlotRegistry::with_defaults();
                let mut plugins = state.plugins.lock().unwrap();
                plugins.load_all(store.plugins_dir(), &cfg.plugins, &mut registry)
            };

            *state.config.write().unwrap() = cfg.clone();

            apply_theme(app, &state);
            crate::tabs::emit_tabs(app, &state);

            let show_toast = cfg.flags.get("notify_reload").copied().unwrap_or(false);
            if rollback_error.is_some() || !warnings.is_empty() || show_toast {
                let mut toast = String::from("Config reloaded");
                let mut kind = "info";
                if let Some(e) = rollback_error {
                    toast = format!("Reload failed, rolled back: {e}");
                    kind = "error";
                } else if !warnings.is_empty() {
                    toast = format!("Config reloaded ({} warning(s))", warnings.len());
                }
                app.emit("umbra://toast", serde_json::json!({ "message": toast, "kind": kind }))
                    .ok();
            }
            for w in warnings {
                app.emit(
                    "umbra://console",
                    serde_json::json!({ "level": "warn", "message": w }),
                )
                .ok();
            }
        }
        Err(e) => {
            app.emit(
                "umbra://toast",
                serde_json::json!({ "message": format!("Config rejected: {e}"), "kind": "error" }),
            )
            .ok();
        }
    }
}

/// Push the full resolved theme + layout into the chrome webview.
pub fn apply_theme(app: &AppHandle, state: &tauri::State<crate::state::AppState>) {
    let cfg = resolved_config(state);
    let css = crate::theme::css_variables(&cfg.theme.tokens);
    let payload = serde_json::json!({
        "css": css,
        "mode": cfg.theme.mode,
        "layout": cfg.layout.preset,
        "bookmarksBar": cfg.layout.bookmarks_bar,
        "statusBar": cfg.layout.status_bar,
        "sidebarWidth": cfg.layout.sidebar_width,
    });
    debug_log!(
        "apply_theme preset={} mode={} layout={} tokens={}",
        cfg.theme.preset,
        cfg.theme.mode,
        cfg.layout.preset,
        cfg.theme.tokens.len()
    );
    // Pushed as an event so the chrome UI (Svelte) reacts like any other state.
    app.emit("umbra://chrome", payload).ok();
}
