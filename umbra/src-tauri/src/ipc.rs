#[cfg(unix)]
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::net::UnixListener;
use std::path::Path;
#[cfg(unix)]
use tauri::Manager;
use tauri::AppHandle;

/// Control socket (like `hyprctl`) for external scripting and status-bar
/// integration: `echo "tabs" | socat - UNIX-CONNECT:$XDG_CONFIG_HOME/umbra/umbra.sock`
#[cfg(unix)]
pub fn spawn(app: AppHandle) -> Result<(), String> {
    let path = {
        let state = app.state::<crate::state::AppState>();
        state.store.socket_path()
    };
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).map_err(|e| format!("ipc bind: {e}"))?;
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let app = app.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                let mut stream = stream;
                let mut line = String::new();
                while reader.read_line(&mut line).unwrap_or(0) > 0 {
                    let cmd = line.trim().to_string();
                    let resp = handle(&app, &cmd);
                    stream.write_all(resp.as_bytes()).ok();
                    stream.write_all(b"\n").ok();
                    line.clear();
                }
            });
        }
    });
    Ok(())
}

/// Windows has no `UnixListener` in std, so the line-oriented control socket
/// is Linux-only for now; startup continues without it rather than failing.
#[cfg(not(unix))]
pub fn spawn(_app: AppHandle) -> Result<(), String> {
    crate::debug_log!("control socket disabled: unix domain sockets unavailable here");
    Ok(())
}

#[cfg(unix)]
fn handle(app: &AppHandle, cmd: &str) -> String {
    let state = app.state::<crate::state::AppState>();
    let mut parts = cmd.splitn(2, ' ');
    match (parts.next(), parts.next()) {
        (Some("ping"), _) => "pong".into(),
        (Some("tabs"), _) => serde_json::to_string(&crate::tabs::tab_infos(&state)).unwrap_or_default(),
        (Some("open"), Some(url)) => {
            // open in the active tab; use `new <url>` for a new tab
            match crate::tabs::navigate(app, &state, url) {
                Ok(_) => format!("ok: navigating to {url}"),
                Err(e) => format!("err: {e}"),
            }
        }
        (Some("new"), Some(url)) => {
            crate::tabs::create_tab(app, &state, Some(url.to_string()));
            crate::tabs::emit_tabs(app, &state);
            "ok".into()
        }
        (Some("reload"), _) => {
            crate::watcher::reload_everything(app);
            "ok: reload triggered".into()
        }
        (Some("layout"), Some(preset)) => {
            let valid = [
                "compact", "minimal", "sidebar", "tiling", "floating", "zen",
            ]
            .contains(&preset);
            if !valid {
                return "err: unknown preset".into();
            }
            {
                let mut cfg = state.config.write().unwrap();
                cfg.layout.preset = preset.to_string();
            }
            crate::watcher::apply_theme(app, &state);
            crate::tabs::emit_tabs(app, &state);
            format!("ok: layout={preset}")
        }
        (Some("slots"), _) => {
            let registry = state.slots.lock().unwrap();
            serde_json::to_string(&registry.mounted).unwrap_or_default()
        }
        (Some("plugins"), _) => {
            let host = state.plugins.lock().unwrap();
            serde_json::to_string(&host.loaded).unwrap_or_default()
        }
        (Some("bm"), _) => {
            // list bookmarks
            let bm = state.bookmarks.lock().unwrap();
            serde_json::to_string(&bm.items).unwrap_or_default()
        }
        (Some("safe"), _) => format!("{}", state.safe_mode),
        (Some("quit"), _) => {
            crate::session::save(app);
            app.exit(0);
            "ok".into()
        }
        (Some("help"), _) => "commands: ping, tabs, open <url>, new <url>, reload, layout <preset>, slots, plugins, bm, ai, safe, quit".into(),
        _ => "err: unknown command (try 'help')".into(),
    }
}

/// Best-effort cleanup of a stale socket file at startup.
pub fn cleanup_stale(path: &Path) {
    if path.exists() {
        std::fs::remove_file(path).ok();
    }
}
