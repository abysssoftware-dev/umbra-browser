use serde::{Deserialize, Serialize};
use std::fs;
use tauri::{Emitter, Manager};

/// Session persistence (spec: "session/workspace persistence across restarts").
/// Saved to <config>/session.toml on every tab mutation and before exit;
/// restored on boot unless --safe is given.

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TabEntry {
    id: u32,
    url: String,
    title: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct WindowSize {
    width: f64,
    height: f64,
}

#[derive(Default, Serialize, Deserialize)]
struct SessionFile {
    #[serde(default)]
    active: Option<u32>,
    #[serde(default)]
    window: Option<WindowSize>,
    #[serde(default, rename = "tab")]
    tabs: Vec<TabEntry>,
}

fn session_path(store: &crate::config::ConfigStore) -> std::path::PathBuf {
    store.session_file()
}

/// Snapshot open tabs + active id + window size to disk. Best-effort.
pub fn save(app: &tauri::AppHandle) {
    let state = app.state::<crate::state::AppState>();
    let mgr = state.tabs.lock().unwrap();
    let mut file = SessionFile {
        active: mgr.active,
        ..Default::default()
    };
    for t in &mgr.tabs {
        file.tabs.push(TabEntry {
            id: t.id,
            url: t.history[t.hist_idx].clone(),
            title: t.title.clone(),
        });
    }
    drop(mgr);
    if let Some(win) = app.get_window("main") {
        // inner_size/scale_factor are GTK calls - must run on the main thread
        // (save() runs on command workers, the ipc socket and the watcher too).
        let size = crate::maindispatch::on_main(app, move || -> Option<(f64, f64)> {
            let size = win.inner_size().ok()?;
            let scale = win.scale_factor().unwrap_or(1.0);
            let logical = size.to_logical(scale);
            Some((logical.width, logical.height))
        });
        file.window = size.map(|(width, height)| WindowSize { width, height });
    }
    if let Ok(s) = toml::to_string_pretty(&file) {
        fs::write(session_path(&state.store), s).ok();
    }
}

/// Restored session data (window size used pre-window-build in setup).
pub struct Session {
    pub width: f64,
    pub height: f64,
    pub tabs: Vec<(u32, String)>, // (saved id, url)
    pub active: Option<u32>,
}

pub fn load(store: &crate::config::ConfigStore) -> Option<Session> {
    let raw = fs::read_to_string(session_path(store)).ok()?;
    let f: SessionFile = match toml::from_str(&raw) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[umbra] session.toml parse failed, starting fresh: {e}");
            return None;
        }
    };
    if f.tabs.is_empty() {
        return None;
    }
    let (w, h) = f
        .window
        .map(|x| (x.width.max(640.0), x.height.max(400.0)))
        .unwrap_or((1280.0, 800.0));
    Some(Session {
        width: w,
        height: h,
        tabs: f.tabs.into_iter().map(|t| (t.id, t.url)).collect(),
        active: f.active,
    })
}

/// Reopen the saved tabs. Safe mode ignores the session entirely.
pub fn restore(app: &tauri::AppHandle) -> bool {
    let state = app.state::<crate::state::AppState>();
    let Some(session) = load(&state.store) else {
        return false;
    };
    let mut mapping: Vec<(u32, u32)> = Vec::new(); // (saved id, new id)
    for (saved_id, url) in &session.tabs {
        let new_id = crate::tabs::create_tab(app, &state, Some(url.clone()));
        mapping.push((*saved_id, new_id));
    }
    if let Some(active_saved) = session.active {
        if let Some((_, new_id)) = mapping.iter().find(|(s, _)| *s == active_saved) {
            let _ = crate::tabs::activate_tab(app, &state, *new_id);
        }
    }
    app.emit(
        "umbra://toast",
        serde_json::json!({"message": format!("Restored {} tab(s)", session.tabs.len()), "kind": "info"}),
    )
    .ok();
    true
}
