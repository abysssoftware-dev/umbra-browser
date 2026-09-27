use serde::{Deserialize, Serialize};
use std::fs;

/// Bookmarks - plain TOML store under the config dir, user-editable like
/// everything else in Umbra (dotfiles philosophy).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: u32,
    pub title: String,
    pub url: String,
}

#[derive(Default, Serialize, Deserialize)]
struct BookmarksFile {
    #[serde(default = "default_next_id")]
    next_id: u32,
    #[serde(rename = "bookmark", default)]
    items: Vec<Bookmark>,
}

fn default_next_id() -> u32 {
    1
}

/// In-memory bookmark state held in AppState.
pub struct Bookmarks {
    pub next_id: u32,
    pub items: Vec<Bookmark>,
}

impl Default for Bookmarks {
    fn default() -> Self {
        Bookmarks { next_id: 1, items: vec![] }
    }
}

impl Bookmarks {
    pub fn load(store: &crate::config::ConfigStore) -> Self {
        let path = store.bookmarks_file();
        let Ok(raw) = fs::read_to_string(&path) else {
            return Bookmarks::default();
        };
        match toml::from_str::<BookmarksFile>(&raw) {
            Ok(f) => Bookmarks {
                next_id: f.next_id.max(1),
                items: f.items,
            },
            Err(e) => {
                eprintln!("[umbra] bookmarks.toml parse failed (starting empty): {e}");
                Bookmarks::default()
            }
        }
    }

    pub fn save(&self, store: &crate::config::ConfigStore) {
        let f = BookmarksFile {
            next_id: self.next_id,
            items: self.items.clone(),
        };
        if let Ok(s) = toml::to_string_pretty(&f) {
            fs::write(store.bookmarks_file(), s).ok();
        }
    }

    pub fn add(&mut self, title: String, url: String) -> Bookmark {
        // dedupe by URL - re-bookmarking moves/updates the entry
        self.items.retain(|b| b.url != url);
        let bm = Bookmark { id: self.next_id, title, url };
        self.next_id += 1;
        let ret = bm.clone();
        self.items.push(bm);
        ret
    }

    /// Merge bookmarks parsed from a remote TOML payload (sync_pull).
    /// Existing URLs win - remote entries are only added when new.
    pub fn merge_raw(&mut self, raw: &str) -> Result<usize, String> {
        let f: BookmarksFile =
            toml::from_str(raw).map_err(|e| format!("remote bookmarks invalid: {e}"))?;
        let known: std::collections::HashSet<String> =
            self.items.iter().map(|b| b.url.clone()).collect();
        let mut merged = 0usize;
        for b in f.items {
            if known.contains(&b.url) {
                continue;
            }
            self.items.push(Bookmark { id: self.next_id, ..b });
            self.next_id += 1;
            merged += 1;
        }
        Ok(merged)
    }
}
