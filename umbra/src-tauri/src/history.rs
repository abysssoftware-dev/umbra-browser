use serde::{Deserialize, Serialize};
use std::fs;

/// Browsing history. Records every successful navigation, dedupes
/// consecutive same-URL visits, capped so the file stays small.
pub const HISTORY_CAP: usize = 5000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    /// Unix timestamp (seconds).
    pub ts: u64,
}

#[derive(Default)]
pub struct History {
    /// Most recent first.
    pub entries: Vec<HistoryEntry>,
}

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl History {
    pub fn load(store: &crate::config::ConfigStore) -> Self {
        let Ok(raw) = fs::read_to_string(store.history_file()) else {
            return History::default();
        };
        match toml::from_str::<HistoryFile>(&raw) {
            Ok(f) => History { entries: f.entries },
            Err(e) => {
                eprintln!("[umbra] history.toml parse failed (starting empty): {e}");
                History::default()
            }
        }
    }

    pub fn save(&self, store: &crate::config::ConfigStore) {
        let f = HistoryFile { entries: self.entries.clone() };
        if let Ok(s) = toml::to_string_pretty(&f) {
            fs::write(store.history_file(), s).ok();
        }
    }

    /// Record a visit (most recent first). Consecutive same-URL entries are
    /// deduped (refresh/back-forward doesn't spam the log).
    pub fn record(&mut self, url: &str, title: &str) {
        if url.starts_with("umbra://") {
            return; // internal pages aren't browsing history
        }
        if self.entries.first().map(|e| e.url == url).unwrap_or(false) {
            return;
        }
        self.entries.insert(
            0,
            HistoryEntry { url: url.to_string(), title: title.to_string(), ts: now_ts() },
        );
        self.entries.truncate(HISTORY_CAP);
    }

    /// Backfill the title once the page-load hook knows it.
    pub fn set_title(&mut self, url: &str, title: &str) {
        if title.is_empty() {
            return;
        }
        if let Some(e) = self.entries.iter_mut().find(|e| e.url == url && e.title.is_empty()) {
            e.title = title.to_string();
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[derive(Default, Serialize, Deserialize)]
struct HistoryFile {
    #[serde(default)]
    entries: Vec<HistoryEntry>,
}

/// App-level helper for the page-load hook in tabs.rs.
pub fn record_visit(app: &tauri::AppHandle, url: &str, title: &str) {
    use tauri::Manager;
    let state = app.state::<crate::state::AppState>();
    state.history.lock().unwrap().record(url, title);
}
