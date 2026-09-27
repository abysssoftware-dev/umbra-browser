//! Browser data import (pass-2). Detects installed browser profiles at the
//! default XDG paths and imports bookmarks + history. Passwords are
//! intentionally NOT imported (no vault exists; importing plaintext passwords
//! would be unsafe per spec).

use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

/// One detectable browser profile.
#[derive(Debug, Clone, Serialize)]
pub struct ImportSource {
    pub browser: String,
    pub profile: String,
    pub has_bookmarks: bool,
    pub has_history: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    pub bookmarks_imported: u32,
    pub history_imported: u32,
}

/// Chromium-family base dirs relative to ~/.config: (relative path, browser id).
const CHROMIUM_DIRS: &[(&str, &str)] = &[
    ("google-chrome", "chrome"),
    ("chromium", "chromium"),
    ("BraveSoftware/Brave-Browser", "brave"),
    ("microsoft-edge", "edge"),
    ("Arc", "arc"),
    ("Vivaldi", "vivaldi"),
    ("Opera", "opera"),
    ("opera-stable", "opera"),
    ("YandexBrowser", "yandex"),
    ("BraveSoftware/Brave-Browser-Beta", "brave-beta"),
    ("BraveSoftware/Brave-Browser-Dev", "brave-dev"),
    ("BraveSoftware/Brave-Browser-Nightly", "brave-nightly"),
    ("google-chrome-beta", "chrome-beta"),
    ("google-chrome-unstable", "chrome-dev"),
    ("chromium-browser", "chromium"),
    ("epiphany", "epiphany"),
    ("vivaldi-snapshot", "vivaldi-snapshot"),
];

/// Firefox-family base dirs relative to home: (relative path, browser id).
const FIREFOX_DIRS: &[(&str, &str)] = &[
    (".mozilla/firefox", "firefox"),
    (".mozilla/waterfox", "waterfox"),
    (".librewolf/librewolf", "librewolf"),
    (".zen/zen", "zen"),
    (".floorp/floorp", "floorp"),
    (".ghostery/ghostery", "ghostery"),
    (".mozilla/seamonkey", "seamonkey"),
    (".basilisk/basilisk", "basilisk"),
    (".palemoon/palemoon", "palemoon"),
];

/// Scan the default install paths. Missing dirs are silently omitted -
/// detection never errors.
pub fn detect() -> Vec<ImportSource> {
    let mut out = Vec::new();
    let Some(home) = dirs::home_dir() else { return out };
    let config = dirs::config_dir().unwrap_or_else(|| home.join(".config"));

    for (rel, browser) in CHROMIUM_DIRS {
        let Ok(entries) = fs::read_dir(config.join(rel)) else { continue };
        for entry in entries.flatten() {
            let pdir = entry.path();
            if !pdir.is_dir() {
                continue;
            }
            let has_bookmarks = pdir.join("Bookmarks").is_file();
            let has_history = pdir.join("History").is_file();
            if has_bookmarks || has_history {
                out.push(ImportSource {
                    browser: browser.to_string(),
                    profile: pdir.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                    has_bookmarks,
                    has_history,
                });
            }
        }
    }

    // Firefox-family: scan multiple profile locations
    for (rel, browser) in FIREFOX_DIRS {
        let base = home.join(rel);
        let Ok(entries) = fs::read_dir(&base) else { continue };
        for entry in entries.flatten() {
            let pdir = entry.path();
            if !pdir.is_dir() {
                continue;
            }
            let places = pdir.join("places.sqlite");
            if !(places.is_file() || pdir.join("prefs.js").is_file()) {
                continue;
            }
            out.push(ImportSource {
                browser: browser.to_string(),
                profile: pdir.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                has_bookmarks: places.is_file(),
                has_history: places.is_file(),
            });
        }
    }

    // Arc browser (macOS/Linux variant)
    let arc_config = home.join(".config/Arc");
    if let Ok(entries) = fs::read_dir(&arc_config) {
        for entry in entries.flatten() {
            let pdir = entry.path();
            if !pdir.is_dir() {
                continue;
            }
            let has_bookmarks = pdir.join("Bookmarks").is_file();
            let has_history = pdir.join("History").is_file();
            if has_bookmarks || has_history {
                out.push(ImportSource {
                    browser: "arc".into(),
                    profile: pdir.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                    has_bookmarks,
                    has_history,
                });
            }
        }
    }

    // Helium browser
    let helium_config = home.join(".config/Helium");
    if let Ok(entries) = fs::read_dir(&helium_config) {
        for entry in entries.flatten() {
            let pdir = entry.path();
            if !pdir.is_dir() {
                continue;
            }
            let has_bookmarks = pdir.join("Bookmarks").is_file();
            let has_history = pdir.join("History").is_file();
            if has_bookmarks || has_history {
                out.push(ImportSource {
                    browser: "helium".into(),
                    profile: pdir.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                    has_bookmarks,
                    has_history,
                });
            }
        }
    }

    out
}

fn chromium_base(profile: &str, rel: &str) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("cannot resolve home dir")?;
    let config = dirs::config_dir().unwrap_or_else(|| home.join(".config"));
    let dir = config.join(rel).join(profile);
    if !dir.is_dir() {
        return Err(format!("profile '{}' not found under {}", profile, config.join(rel).display()));
    }
    Ok(dir)
}

fn firefox_base(profile: &str) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("cannot resolve home dir")?;
    let dir = home.join(".mozilla/firefox").join(profile);
    if !dir.is_dir() {
        return Err(format!("firefox profile '{}' not found", profile));
    }
    Ok(dir)
}

/// Copy a sqlite db (+ its -wal side file when present) to a unique temp path
/// so we never open (or lock) the live browser database.
fn copy_db_to_temp(src: &Path, tag: &str) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!(
        "umbra-import-{tag}-{}-{nanos}",
        std::process::id()
    ));
    fs::copy(src, &tmp).map_err(|e| format!("cannot copy {}: {e}", src.display()))?;
    // Copy the WAL too so recently-written rows are visible to our reader.
    if let Some(name) = src.file_name() {
        let wal = src.with_file_name(format!("{}-wal", name.to_string_lossy()));
        if wal.is_file() {
            fs::copy(&wal, tmp.with_file_name(format!("{}-wal", tmp.file_name().unwrap_or_default().to_string_lossy()))).ok();
        }
    }
    Ok(tmp)
}

fn cleanup_temp(path: &Path) {
    fs::remove_file(path).ok();
    if let Some(name) = path.file_name() {
        fs::remove_file(path.with_file_name(format!("{}-wal", name.to_string_lossy()))).ok();
    }
}

/// Recursively walk Chromium's nested Bookmarks JSON collecting (name, url).
fn walk_chromium_node(node: &serde_json::Value, out: &mut Vec<(String, String)>) {
    if node.get("type").and_then(|t| t.as_str()) == Some("url") {
        if let (Some(name), Some(url)) = (
            node.get("name").and_then(|v| v.as_str()),
            node.get("url").and_then(|v| v.as_str()),
        ) {
            out.push((name.to_string(), url.to_string()));
        }
    }
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for child in children {
            walk_chromium_node(child, out);
        }
    }
}

/// Merge imported history rows (most recent first) with dedupe by URL.
/// Returns how many new entries landed.
fn merge_history(
    state: &crate::state::AppState,
    rows: Vec<(String, String, u64)>,
) -> u32 {
    let mut hist = state.history.lock().unwrap();
    let known: HashSet<String> = hist.entries.iter().map(|e| e.url.clone()).collect();
    let mut added = 0u32;
    let mut seen: HashSet<String> = HashSet::new();
    for (url, title, ts) in rows {
        if known.contains(&url) || seen.contains(&url) {
            continue;
        }
        seen.insert(url.clone());
        hist.entries.push(crate::history::HistoryEntry { url, title, ts });
        added += 1;
    }
    hist.entries.sort_by(|a, b| b.ts.cmp(&a.ts));
    hist.entries.truncate(crate::history::HISTORY_CAP);
    hist.save(&state.store);
    added
}

fn emit_import_state(app: &AppHandle, toast: Option<String>) {
    let state = app.state::<crate::state::AppState>();
    let items = state.bookmarks.lock().unwrap().items.clone();
    app.emit("umbra://state", serde_json::json!({ "bookmarks": items })).ok();
    if let Some(message) = toast {
        app.emit("umbra://toast", serde_json::json!({"message": message, "kind": "ok"})).ok();
    }
}

/// Import bookmarks and/or history from one detected profile.
pub fn import_browser(
    app: &AppHandle,
    browser: &str,
    profile: &str,
    want_bookmarks: bool,
    want_history: bool,
) -> Result<ImportResult, String> {
    let mut result = ImportResult { bookmarks_imported: 0, history_imported: 0 };

    match browser {
        "chrome" | "chromium" | "brave" | "edge" => {
            let rel = CHROMIUM_DIRS
                .iter()
                .find(|(_, b)| *b == browser)
                .map(|(r, _)| *r)
                .ok_or_else(|| format!("unknown browser '{browser}'"))?;
            let dir = chromium_base(profile, rel)?;

            if want_bookmarks {
                let raw = fs::read_to_string(dir.join("Bookmarks"))
                    .map_err(|e| format!("cannot read Bookmarks JSON: {e}"))?;
                let json: serde_json::Value =
                    serde_json::from_str(&raw).map_err(|e| format!("Bookmarks parse error: {e}"))?;
                let mut found = Vec::new();
                if let Some(roots) = json.get("roots").and_then(|r| r.as_object()) {
                    for (_root, node) in roots {
                        walk_chromium_node(node, &mut found);
                    }
                }
                result.bookmarks_imported = add_bookmarks(app, found)?;
            }
            if want_history {
                let src = dir.join("History");
                if !src.is_file() {
                    return Err(format!("History db not found at {}", src.display()));
                }
                let tmp = copy_db_to_temp(&src, "chromium-history")?;
                let count = (|| -> Result<u32, String> {
                    let conn = rusqlite::Connection::open_with_flags(
                        &tmp,
                        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                    )
                    .map_err(|e| format!("cannot open History copy: {e}"))?;
                    let mut stmt = conn
                        .prepare(
                            "SELECT url, title, visit_count, last_visit_time FROM urls \
                             WHERE visit_count > 0 ORDER BY last_visit_time DESC LIMIT 2000",
                        )
                        .map_err(|e| format!("history query failed: {e}"))?;
                    let rows = stmt
                        .query_map([], |row| {
                            // Chromium timestamps are µs since 1601-01-01.
                            let t: i64 = row.get::<_, Option<i64>>(3)?.unwrap_or(0);
                            let ts = (t / 1_000_000 - 11_644_473_600).max(0) as u64;
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                                ts,
                            ))
                        })
                        .map_err(|e| format!("history query failed: {e}"))?;
                    let mut collected = Vec::new();
                    for r in rows {
                        collected.push(r.map_err(|e| format!("history row read failed: {e}"))?);
                    }
                    Ok(merge_history(&app.state::<crate::state::AppState>(), collected))
                })();
                cleanup_temp(&tmp);
                result.history_imported = count?;
            }
        }
        "firefox" => {
            let dir = firefox_base(profile)?;
            let src = dir.join("places.sqlite");
            if !src.is_file() {
                return Err(format!("places.sqlite not found at {}", src.display()));
            }
            let tmp = copy_db_to_temp(&src, "firefox-places")?;
            let counts = (|| -> Result<(u32, u32), String> {
                let conn = rusqlite::Connection::open_with_flags(
                    &tmp,
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                )
                .map_err(|e| format!("cannot open places.sqlite copy: {e}"))?;

                let bm_count = if want_bookmarks {
                    let mut stmt = conn
                        .prepare(
                            "SELECT p.url, b.title FROM moz_bookmarks b \
                             JOIN moz_places p ON b.fk = p.id WHERE b.type = 1",
                        )
                        .map_err(|e| format!("bookmarks query failed: {e}"))?;
                    let rows = stmt
                        .query_map([], |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                            ))
                        })
                        .map_err(|e| format!("bookmarks query failed: {e}"))?;
                    let mut found = Vec::new();
                    for r in rows {
                        found.push(r.map_err(|e| format!("bookmark row read failed: {e}"))?);
                    }
                    add_bookmarks(app, found)?
                } else {
                    0
                };

                let hist_count = if want_history {
                    let mut stmt = conn
                        .prepare(
                            "SELECT url, COALESCE(title, ''), COALESCE(last_visit_date, 0) \
                             FROM moz_places WHERE hidden = 0 AND url LIKE 'http%' \
                             ORDER BY last_visit_date DESC LIMIT 2000",
                        )
                        .map_err(|e| format!("history query failed: {e}"))?;
                    let rows = stmt
                        .query_map([], |row| {
                            // Firefox PRTime is µs since the unix epoch.
                            let us: i64 = row.get(2)?;
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                                (us / 1_000_000).max(0) as u64,
                            ))
                        })
                        .map_err(|e| format!("history query failed: {e}"))?;
                    let mut collected = Vec::new();
                    for r in rows {
                        collected.push(r.map_err(|e| format!("history row read failed: {e}"))?);
                    }
                    merge_history(&app.state::<crate::state::AppState>(), collected)
                } else {
                    0
                };
                Ok((bm_count, hist_count))
            })();
            cleanup_temp(&tmp);
            let (bm, hist) = counts?;
            result.bookmarks_imported = bm;
            result.history_imported = hist;
        }
        other => return Err(format!("unknown browser '{other}' (chrome/chromium/brave/edge/firefox)")),
    }

    emit_import_state(
        app,
        Some(format!(
            "Imported {} bookmark(s), {} history entr(ies)",
            result.bookmarks_imported, result.history_imported
        )),
    );
    Ok(result)
}

/// Add bookmark (title,url) pairs through the normal store (dedupes by URL).
fn add_bookmarks(app: &AppHandle, found: Vec<(String, String)>) -> Result<u32, String> {
    let state = app.state::<crate::state::AppState>();
    let mut bms = state.bookmarks.lock().unwrap();
    let mut known: HashSet<String> = bms.items.iter().map(|b| b.url.clone()).collect();
    let mut added = 0u32;
    for (title, url) in found {
        if url.is_empty() || !known.insert(url.clone()) {
            continue;
        }
        bms.add(if title.is_empty() { url.clone() } else { title }, url);
        added += 1;
    }
    bms.save(&state.store);
    Ok(added)
}
