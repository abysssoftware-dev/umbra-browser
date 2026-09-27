use std::sync::{Mutex, RwLock};

use crate::config::Config;
use crate::plugins::PluginHost;
use crate::slots::SlotRegistry;
use crate::tabs::TabManager;

pub struct AppState {
    pub config: RwLock<Config>,
    pub tabs: Mutex<TabManager>,
    pub slots: Mutex<SlotRegistry>,
    pub plugins: Mutex<PluginHost>,
    pub store: crate::config::ConfigStore,
    pub safe_mode: bool,
    /// Bookmarks store (bookmarks.toml backing).
    pub bookmarks: Mutex<crate::bookmarks::Bookmarks>,
    /// Browsing history (history.toml backing).
    pub history: Mutex<crate::history::History>,
    /// Staged .umbrapkg awaiting user approval (PkgDiff view).
    pub pending_pkg: Mutex<Option<crate::pkg::PendingPkg>>,
    /// Picture-in-picture window state (best-effort; the window registry is
    /// re-checked on every pip_toggle).
    pub pip_open: std::sync::atomic::AtomicBool,
    /// Last gist-sync failure, surfaced through sync_status.
    pub sync_error: Mutex<Option<String>>,
}
