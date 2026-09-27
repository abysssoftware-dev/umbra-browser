//! Picture-in-picture (pass-2): a small frameless always-on-top window
//! mirroring the active tab's URL.

use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// Toggle the "pip" window. Returns the new on/off state.
///
/// Window create/close are GTK calls; the whole body runs on the main thread
/// (toggle() itself may be invoked from command workers or the ipc socket).
pub fn toggle(app: &AppHandle) -> Result<bool, String> {
    let app2 = app.clone();
    crate::maindispatch::on_main(app, move || toggle_main(&app2))
}

fn toggle_main(app: &AppHandle) -> Result<bool, String> {
    // Liveness is checked best-effort against the actual window registry each
    // call, so a manually-closed PiP window never leaves stale state behind.
    if let Some(existing) = app.get_window("pip") {
        existing.close().ok();
        app.state::<crate::state::AppState>()
            .pip_open
            .store(false, Ordering::Relaxed);
        return Ok(false);
    }

    let state = app.state::<crate::state::AppState>();
    let url_str = {
        let mgr = state.tabs.lock().unwrap();
        let id = mgr.active.ok_or("no active tab")?;
        let tab = mgr.get(id).ok_or("no active tab")?;
        tab.history[tab.hist_idx].clone()
    };
    if !url_str.starts_with("http://") && !url_str.starts_with("https://") {
        return Err("picture-in-picture needs an http(s) page".into());
    }
    let url: tauri::Url = url_str.parse().map_err(|e| format!("bad url: {e}"))?;
    WebviewWindowBuilder::new(app, "pip", WebviewUrl::External(url))
        .title("Umbra PiP")
        .inner_size(420.0, 240.0)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .transparent(false)
        .build()
        .map_err(|e| format!("cannot open pip window: {e}"))?;
    state.pip_open.store(true, Ordering::Relaxed);
    Ok(true)
}
