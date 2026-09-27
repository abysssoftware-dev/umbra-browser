//! Main-thread dispatch helper.
//!
//! GTK/WebKit calls (window/webview creation, navigation, bounds, show/hide,
//! eval…) must happen on the process main thread; calling them from Tauri
//! command worker threads, the IPC socket threads or the config watcher
//! deadlocks forever. Every window/webview mutation funnels through [`on_main`].

use std::sync::mpsc;

/// True when running on the process main thread (Tauri's event loop).
fn is_main_thread() -> bool {
    std::thread::current().name() == Some("main")
}

/// Run `f` on the main thread and return its result. If already on the main
/// thread, runs inline (a queued closure would never run - the loop is busy
/// with us - so queueing would self-deadlock).
///
/// LOCK ORDERING: never call this while holding a Mutex/RwLock that an
/// on_main closure (or any other main-thread work) also takes; acquire locks
/// INSIDE the closure or drop them before dispatching, or the two threads
/// deadlock against each other.
pub fn on_main<F, R>(app: &tauri::AppHandle, f: F) -> R
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    if is_main_thread() {
        return f();
    }
    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        // If f panics the channel closes; recv below reports it.
        let _ = tx.send(f());
    })
    .unwrap_or_else(|e| panic!("on_main: event loop unavailable: {e}"));
    rx.recv()
        .unwrap_or_else(|_| panic!("on_main: closure panicked"))
}
