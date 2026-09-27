use serde::Serialize;
use tauri::webview::Webview;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewBuilder, Wry};
use crate::debug_log;

/// Desktop Chrome UA so sites serve their desktop layout instead of a mobile
/// variant (WebKitGTK's default UA gets mis-detected as mobile by some sites).
/// Override at launch with UMBRA_USER_AGENT if a specific site needs it.
/// Exact desktop Firefox on Linux UA (byte-for-byte what Firefox sends, no
/// AppleWebKit/Safari/WebKit tokens). Firefox sends no User-Agent Client Hints,
/// so both server-side UA sniffing and client-side JS see desktop Firefox.
/// Bump both 140.0 values together to track the current Firefox release.
const DESKTOP_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0";

/// Client-side reinforcement: make navigator.* read as the configured UA and
/// remove the Chromium-only userAgentData (client hints) object so JS
/// engine-detection also sees the chosen browser, not WebKitGTK. Runs before
/// every page script via the webview initialization script. The UA string is
/// injected so it always matches the actual User-Agent header.
fn spoof_script(ua: &str) -> String {
    let ua_json = serde_json::to_string(ua).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"
(function () {{
  var ua = {ua_json};
  var props = {{
    userAgent: ua,
    appVersion: "5.0 (X11)",
    vendor: "",
    platform: "Linux x86_64",
    oscpu: "Linux x86_64",
    product: "Gecko",
    productSub: "20100101",
    language: (navigator.language || "en-US"),
    languages: (navigator.languages || ["en-US"])
  }};
  Object.keys(props).forEach(function (k) {{
    try {{
      Object.defineProperty(navigator, k, {{ get: function () {{ return props[k]; }}, configurable: true }});
    }} catch (e) {{}}
  }});
  try {{
    Object.defineProperty(navigator, "userAgentData", {{ get: function () {{ return undefined; }}, configurable: true }});
  }} catch (e) {{}}
}})();
"#
    )
}

// GTK/WebKit calls only work on the main thread - every webview/window
// mutation below goes through on_main (see maindispatch.rs for the rules).
use crate::maindispatch::on_main;

/// Start of the most recent `Started` page-load event (with its URL), so the
/// `Finished` hook can log how long a page actually took. Diagnostics for
/// "some pages are slow to load" reports - see ~/.config/umbra/debug.log.
static LOAD_START: std::sync::Mutex<Option<(std::time::Instant, String)>> =
    std::sync::Mutex::new(None);

#[derive(Debug, Clone, Serialize)]
pub struct TabInfo {
    pub id: u32,
    pub title: String,
    pub url: String,
    pub active: bool,
    pub loading: bool,
}

pub struct Tab {
    pub id: u32,
    pub title: String,
    pub history: Vec<String>,
    pub hist_idx: usize,
    pub webview: Option<Webview<Wry>>,
    /// True once the webview has been created and positioned at least once.
    pub realized: bool,
}

#[derive(Default)]
pub struct TabManager {
    next_id: u32,
    pub tabs: Vec<Tab>,
    pub active: Option<u32>,
    /// Split view: secondary (right-half) tab id. None = split off.
    pub split: Option<u32>,
    /// Content-area rect in physical pixels, reported by the chrome UI.
    pub content: (f64, f64, f64, f64),
    /// Whether tab webviews are currently visible (false while a modal/settings
    /// overlay is open). Drives the chrome input-shape carve-out.
    pub webviews_visible: bool,
    /// True while the hamburger dropdown (or any in-content chrome popup) is
    /// open: the chrome must capture all pointer events then (the popup can
    /// overlap the content area, which is otherwise click-through to the tab).
    pub menu_open: bool,
}

impl TabManager {
    pub fn with_content(content: (f64, f64, f64, f64)) -> Self {
        TabManager {
            content,
            webviews_visible: true,
            menu_open: false,
            ..Default::default()
        }
    }

    fn alloc_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    pub fn get(&self, id: u32) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    fn get_mut(&mut self, id: u32) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn active_id(&self) -> Option<u32> {
        self.active
    }
}

/// Normalize a user-entered URL into something navigable.
pub fn normalize_url(input: &str) -> String {
    let s = input.trim();
    if s.is_empty() {
        return "umbra://newtab".into();
    }
    if s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("file://")
        || s.starts_with("umbra://")
    {
        return s.to_string();
    }
    // looks like a host/domain?
    if s.contains('.') && !s.contains(' ') {
        return format!("https://{s}");
    }
    // else treat as a search - route through DuckDuckGo (no tracking)
    format!(
        "https://duckduckgo.com/?q={}",
        urlencoding_lite(s)
    )
}

fn urlencoding_lite(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub struct TabOutcome {
    pub warnings: Vec<String>,
}

/// Create a new tab. The webview is created lazily on first navigation.
pub fn create_tab(app: &AppHandle, state: &crate::state::AppState, url: Option<String>) -> u32 {
    // Honor the configured new-tab page when the caller doesn't specify one.
    let target = url.unwrap_or_else(|| {
        state
            .config
            .read()
            .unwrap()
            .newtab
            .target_url()
    });
    let id = {
        let mut mgr = state.tabs.lock().unwrap();
        let id = mgr.alloc_id();
        let url = normalize_url(&target);
        mgr.tabs.push(Tab {
            id,
            title: "New Tab".into(),
            history: vec![url],
            hist_idx: 0,
            webview: None,
            realized: false,
        });
        mgr.active = Some(id);
        id
    };
    debug_log!("create_tab id={id} url={target}");
    // A new tab starts on the new-tab page; nothing to realize yet.
    let _ = app;
    id
}

/// Build the native child webview for a tab and place it in the content area.
fn realize_tab(app: &AppHandle, state: &crate::state::AppState, id: u32) -> Result<(), String> {
    let (url, label, _content) = {
        let mgr = state.tabs.lock().unwrap();
        let tab = mgr.get(id).ok_or("tab not found")?;
        (tab.history[tab.hist_idx].clone(), format!("tab-{id}"), mgr.content)
    };
    if url == "umbra://newtab" {
        return Ok(()); // new-tab page is drawn by chrome; no webview needed yet
    }
    {
        let mgr = state.tabs.lock().unwrap();
        if mgr.get(id).map(|t| t.webview.is_some()).unwrap_or(false) {
            return Ok(());
        }
    }
    let parsed: tauri::Url = url.parse().map_err(|e| format!("bad url: {e}"))?;
    let app_for_load = app.clone();
    // UA priority: UMBRA_USER_AGENT env > configured user_agent > built-in default.
    let ua = std::env::var("UMBRA_USER_AGENT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            let cfg = state.config.read().unwrap();
            let v = cfg.user_agent.trim().to_string();
            if v.is_empty() { None } else { Some(v) }
        })
        .unwrap_or_else(|| DESKTOP_USER_AGENT.to_string());
    let builder = WebviewBuilder::new(label.clone(), WebviewUrl::External(parsed))
        .user_agent(&ua)
        .initialization_script(spoof_script(&ua))
        .on_page_load(move |webview, payload| {
            let load_url = payload.url().to_string();
            let ev = payload.event();
            if ev == tauri::webview::PageLoadEvent::Started {
                *LOAD_START.lock().unwrap() = Some((std::time::Instant::now(), load_url));
                return;
            }
            if ev == tauri::webview::PageLoadEvent::Finished {
                // Time this load (Started -> Finished) when we saw its start;
                // in-page link clicks and bfcache restores may lack a pair.
                let took = LOAD_START
                    .lock()
                    .unwrap()
                    .take()
                    .filter(|(_, u)| *u == load_url)
                    .map(|(t, _)| t.elapsed());
                match took {
                    Some(d) => debug_log!("page load: {load_url} finished in {d:.2?}"),
                    None => debug_log!("page load: {load_url} finished (no matching start)"),
                }
                let host = webview
                    .url()
                    .ok()
                    .and_then(|u| u.host_str().map(|h| h.to_string()))
                    .unwrap_or_else(|| "…".into());
                // Successful navigation → history entry (deduped/capped there).
                crate::history::record_visit(&app_for_load, &load_url, &host);
                crate::tabs::on_title_ready(&app_for_load, host);
            }
        });
    // add_child is a GTK call - dispatch to the main thread with NO locks held.
    let app_create = app.clone();
    let label_for_embed = label.clone();
    let t_realize = std::time::Instant::now();
    on_main(app, move || -> Result<(), String> {
        let window = app_create
            .get_window("main")
            .ok_or("main window missing")?;

        let content = _content;
        let (x, y, w, h) = content;

        // add_child's position/size args are ignored by wry on Linux (it
        // lands in a gtk::Box regardless of what we pass), so the values
        // here don't matter there - real placement happens below via the
        // gtk::Fixed overlay. On macOS/Windows, add_child does real
        // absolute positioning, so pass the real content rect directly.
        #[cfg(target_os = "linux")]
        let add_result = window.add_child(
            builder,
            tauri::PhysicalPosition::new(0, 0),
            tauri::PhysicalSize::new(1, 1),
        );
        #[cfg(not(target_os = "linux"))]
        let add_result = window.add_child(
            builder,
            tauri::PhysicalPosition::new(x.round() as i32, y.round() as i32),
            tauri::PhysicalSize::new((w.round() as i32).max(1), (h.round() as i32).max(1)),
        );
        add_result.map_err(|e| e.to_string())?;

        #[cfg(target_os = "linux")]
        {
            // Move the freshly-created tab webview out of the gtk::Box and
            // into the shared overlay gtk::Fixed, on top of chrome, at the
            // real content rect. This is the piece that was missing -
            // without it the webview stays wherever gtk::Box's sequential
            // layout happened to put it (the empty-gap bug).
            if let Some(wv) = app_create.get_webview(&label_for_embed) {
                let (tabs_fixed, _chrome_fixed) =
                    crate::linux_overlay::get_or_create_fixeds(&window)?;
                crate::linux_overlay::embed(
                    &tabs_fixed,
                    &wv,
                    x.round() as i32,
                    y.round() as i32,
                    w.round() as i32,
                    h.round() as i32,
                )?;
                // Chrome lives in a separate Fixed packed *above* the tabs Fixed, so
                // it always paints over tab content without re-parenting the webview
                // (re-parenting would drop the chrome's alpha and blank the page).

                // Autoplay: wry already builds the webview with
                // AutoplayPolicy::Allow, but flip the WebKit setting explicitly
                // too so media never waits for a user gesture regardless of
                // site heuristics (belt and suspenders for the "videos don't
                // auto-start" report).
                {
                    use webkit2gtk::{SettingsExt, WebViewExt};
                    let _ = wv.with_webview(|pw| {
                        if let Some(settings) = pw.inner().settings() {
                            settings.set_media_playback_requires_user_gesture(false);
                        }
                    });
                }
            }
        }
        Ok(())
    })?;
    debug_log!(
        "realize_tab id={id} webview ready in {:?}",
        t_realize.elapsed()
    );
    // Record the realized webview (cheap handle lookup + short lock).
    {
        let mut mgr = state.tabs.lock().unwrap();
        if let Some(tab) = mgr.get_mut(id) {
            tab.webview = app.get_webview(&label);
            tab.realized = true;
        }
    }
    // A webview realized while an overlay or the menu is open must start with
    // its input region blanked, like its siblings.
    refresh_tab_input(app, state);
    Ok(())
}

/// Called from the page-load hook to update a tab's title.
pub fn on_title_ready(app: &AppHandle, title: String) {
    let state = app.state::<crate::state::AppState>();
    let url = {
        let mut mgr = state.tabs.lock().unwrap();
        let Some(active) = mgr.active else { return };
        let Some(tab) = mgr.get_mut(active) else { return };
        tab.title = title.clone();
        tab.history[tab.hist_idx].clone()
    };
    // Backfill the history entry title now that we know it.
    state.history.lock().unwrap().set_title(&url, &title);
    emit_tabs(app, &state);
}

pub fn navigate(
    app: &AppHandle,
    state: &crate::state::AppState,
    input: &str,
) -> Result<(), String> {
    let t0 = std::time::Instant::now();
    let url = normalize_url(input);
    debug_log!("navigate input={input} -> {url}");
    // Built-in ad/tracker blocker (default-on flag; set_flag("adblock", false) removes it).
    crate::adblock::check_navigation(app, state, &url)?;
    let id = {
        let mut mgr = state.tabs.lock().unwrap();
        let id = mgr.active.ok_or("no active tab")?;
        let tab = mgr.get_mut(id).unwrap();
        tab.history.truncate(tab.hist_idx + 1);
        tab.history.push(url.clone());
        tab.hist_idx += 1;
        id
    };
    realize_tab(app, state, id)?;
    goto_current(app, state, id)?;
    debug_log!("navigate dispatched url={url} total={:?}", t0.elapsed());
    emit_tabs(app, state);
    Ok(())
}

fn goto_current(app: &AppHandle, state: &crate::state::AppState, id: u32) -> Result<(), String> {
    // Snapshot the url + webview handle under the lock, then drop it before
    // any main-thread dispatch (never dispatch while holding tabs lock).
    let (url, wv) = {
        let mgr = state.tabs.lock().unwrap();
        let tab = mgr.get(id).ok_or("tab not found")?;
        (tab.history[tab.hist_idx].clone(), tab.webview.clone())
    };
    match wv {
        Some(wv) if url == "umbra://newtab" => {
            on_main(app, move || {
                wv.hide().ok();
            });
        }
        Some(wv) => {
            let parsed: tauri::Url = url.parse().map_err(|e| format!("bad url: {e}"))?;
            on_main(app, move || -> Result<(), String> {
                wv.navigate(parsed).map_err(|e| e.to_string())?;
                wv.show().ok();
                Ok(())
            })?;
        }
        None if url == "umbra://newtab" => {}
        None => {
            realize_tab(app, state, id)?;
            let wv = {
                let mgr = state.tabs.lock().unwrap();
                mgr.get(id).and_then(|t| t.webview.clone())
            };
            if let Some(wv) = wv {
                on_main(app, move || {
                    wv.show().ok();
                });
            }
        }
    }
    Ok(())
}

pub fn go_back(app: &AppHandle, state: &crate::state::AppState) -> Result<(), String> {
    let id = {
        let mut mgr = state.tabs.lock().unwrap();
        let id = mgr.active.ok_or("no active tab")?;
        let tab = mgr.get_mut(id).unwrap();
        if tab.hist_idx > 0 {
            tab.hist_idx -= 1;
            Some(id)
        } else {
            None
        }
    };
    if let Some(id) = id {
        goto_current(app, state, id)?;
        emit_tabs(app, state);
    }
    Ok(())
}

pub fn go_forward(app: &AppHandle, state: &crate::state::AppState) -> Result<(), String> {
    let id = {
        let mut mgr = state.tabs.lock().unwrap();
        let id = mgr.active.ok_or("no active tab")?;
        let tab = mgr.get_mut(id).unwrap();
        if tab.hist_idx + 1 < tab.history.len() {
            tab.hist_idx += 1;
            Some(id)
        } else {
            None
        }
    };
    if let Some(id) = id {
        goto_current(app, state, id)?;
        emit_tabs(app, state);
    }
    Ok(())
}

pub fn reload(app: &AppHandle, state: &crate::state::AppState) -> Result<(), String> {
    let id = mgr_active(state)?;
    goto_current(app, state, id)
}

fn mgr_active(state: &crate::state::AppState) -> Result<u32, String> {
    state
        .tabs
        .lock()
        .unwrap()
        .active
        .ok_or_else(|| "no active tab".into())
}

pub fn activate_tab(app: &AppHandle, state: &crate::state::AppState, id: u32) -> Result<(), String> {
    let prev = {
        let mut mgr = state.tabs.lock().unwrap();
        if mgr.get(id).is_none() {
            return Err("tab not found".into());
        }
        let prev = mgr.active;
        mgr.active = Some(id);
        // While split is on, activating any tab makes THAT tab primary and the
        // previous primary slides over to secondary.
        if mgr.split.is_some() && prev.is_some() && prev != Some(id) {
            mgr.split = prev;
        }
        prev
    };
    debug_log!("activate_tab id={id} prev={prev:?} split={:?}", state.tabs.lock().unwrap().split);
    if let Some(p) = prev {
        if p != id {
            hide_tab(app, state, p);
        }
    }
    show_and_position(app, state, id)?;
    // Split view: place primary + secondary halves (no-op when split is off).
    reposition_all(app, state);
    refresh_chrome_input_shape(app, state);
    emit_tabs(app, state);
    Ok(())
}

fn hide_tab(app: &AppHandle, state: &crate::state::AppState, id: u32) {
    let wv = {
        let mgr = state.tabs.lock().unwrap();
        mgr.get(id).and_then(|t| t.webview.clone())
    };
    if let Some(wv) = wv {
        on_main(app, move || {
            wv.hide().ok();
        });
    }
}

fn show_and_position(app: &AppHandle, state: &crate::state::AppState, id: u32) -> Result<(), String> {
    realize_tab(app, state, id)?;
    position_tab(app, state, id)
}

pub fn close_tab(app: &AppHandle, state: &crate::state::AppState, id: u32) -> Result<(), String> {
    let (do_close, next) = {
        let mut mgr = state.tabs.lock().unwrap();
        let idx = mgr.tabs.iter().position(|t| t.id == id).ok_or("tab not found")?;
        mgr.tabs.remove(idx);
        // Closing a split participant tears down the split (stale secondary).
        if mgr.split == Some(id) || mgr.active == Some(id) {
            mgr.split = None;
        }
        let next = if mgr.tabs.is_empty() {
            None
        } else {
            Some(mgr.tabs[idx.min(mgr.tabs.len() - 1)].id)
        };
        if mgr.active == Some(id) {
            mgr.active = next;
        }
        (!mgr.tabs.is_empty(), next)
    };
    // close the tab's webview if one was realized (GTK call → main thread)
    let label = format!("tab-{id}");
    if let Some(wv) = app.get_webview(&label) {
        on_main(app, move || {
            wv.close().ok();
        });
    }
    if do_close {
        if let Some(n) = next {
            show_and_position(app, state, n)?;
        }
    } else {
        // last tab closed → terminate the whole browser
        debug_log!("last tab closed; exiting application");
        app.exit(0);
        return Ok(());
    }
    emit_tabs(app, state);
    Ok(())
}

pub fn next_tab(app: &AppHandle, state: &crate::state::AppState, delta: i32) {
    let target = {
        let mgr = state.tabs.lock().unwrap();
        if mgr.tabs.is_empty() {
            return;
        }
        let cur = mgr
            .tabs
            .iter()
            .position(|t| Some(t.id) == mgr.active)
            .unwrap_or(0);
        let len = mgr.tabs.len();
        let next = ((cur as i32 + delta).rem_euclid(len as i32)) as usize;
        mgr.tabs[next].id
    };
    let _ = activate_tab(app, state, target);
}

/// Reposition visible webviews into the reported content rect. When split view
/// is active, primary takes the left half and secondary the right half
/// (physical px math; chrome reports full content bounds).
pub fn reposition_all(app: &AppHandle, state: &crate::state::AppState) {
    let (active, split, content) = {
        let mgr = state.tabs.lock().unwrap();
        (mgr.active, mgr.split, mgr.content)
    };
    match (active, split) {
        (Some(a), Some(s)) => {
            let (x, y, w, h) = content;
            position_in_rect(app, state, a, x, y, w / 2.0, h).ok();
            position_in_rect(app, state, s, x + w / 2.0, y, w / 2.0, h).ok();
        }
        (Some(a), None) => {
            position_in_rect(app, state, a, content.0, content.1, content.2, content.3).ok();
        }
        _ => {}
    }
}

/// Toggle "overlay open" state for the native tab webviews (Settings, palette,
/// console, dialogs…). NOTE: this deliberately does NOT gtk-hide() the
/// webviews anymore. With the transparent window and the chrome's translucent
/// overlay backdrops, hiding them left nothing opaque behind the content area,
/// so the desktop showed through instead of the page - the "opening a menu
/// turns the background into the desktop" gripe. Overlay click-through is
/// handled by blanking the tab input regions instead (refresh_tab_input), the
/// same KWin-safe mechanism the hamburger menu uses, and the chrome keeps its
/// cleared input shape so the overlay receives every pointer event.
pub fn set_webviews_visible(app: &AppHandle, state: &crate::state::AppState, visible: bool) {
    debug_log!("set_webviews_visible visible={visible}");
    // Flag first: the refresh helpers below derive from it.
    state.tabs.lock().unwrap().webviews_visible = visible;
    if visible {
        // Recovery path: re-show anything that is hidden (e.g. the active tab
        // was switched away from the new-tab page while overlays existed) and
        // put every visible webview back into the content rect. The show calls
        // are guarded by the tab URL so a hidden new-tab webview stays hidden.
        reposition_all(app, state);
        let (active, split) = {
            let mgr = state.tabs.lock().unwrap();
            (mgr.active, mgr.split)
        };
        if let Some(a) = active {
            let (url, wv) = {
                let mgr = state.tabs.lock().unwrap();
                mgr.get(a)
                    .map(|t| (t.history[t.hist_idx].clone(), t.webview.clone()))
                    .unwrap_or_default()
            };
            if url != "umbra://newtab" {
                if let Some(wv) = wv {
                    on_main(app, move || {
                        wv.show().ok();
                    });
                }
            }
        }
        if let Some(s) = split {
            let (url, wv) = {
                let mgr = state.tabs.lock().unwrap();
                mgr.get(s)
                    .map(|t| (t.history[t.hist_idx].clone(), t.webview.clone()))
                    .unwrap_or_default()
            };
            if url != "umbra://newtab" {
                if let Some(wv) = wv {
                    on_main(app, move || {
                        wv.show().ok();
                    });
                }
            }
        }
    }

    refresh_chrome_input_shape(app, state);
    refresh_tab_input(app, state);
}

/// Keeps the chrome webview's input region in sync with the active tab: when a
/// real web page is active and visible, carve out the content rect so clicks
/// reach the tab below; otherwise (new-tab page, a modal overlay open, or the
/// hamburger dropdown open) let the chrome capture everything.
fn refresh_chrome_input_shape(app: &AppHandle, state: &crate::state::AppState) {
    let (cx, cy, cw, ch, web_active, capture_all) = {
        let mgr = state.tabs.lock().unwrap();
        let (cx, cy, cw, ch) = mgr.content;
        let web_active = mgr
            .active
            .and_then(|id| mgr.tabs.iter().find(|t| t.id == id))
            .map(|t| {
                t.history
                    .get(t.hist_idx)
                    .map(|u| u != "umbra://newtab")
                    .unwrap_or(false)
                    && t.webview.is_some()
            })
            .unwrap_or(false);
        // Capture everything when tab webviews are hidden (modal overlay) or the
        // hamburger dropdown is open (it can overlap the click-through area).
        let capture_all = !mgr.webviews_visible || mgr.menu_open;
        (cx, cy, cw, ch, web_active, capture_all)
    };
    debug_log!(
        "refresh_chrome_input_shape capture_all={capture_all} web_active={web_active} content=({cx},{cy},{cw},{ch})"
    );

    if let Some(chrome) = app.get_webview("chrome") {
        // with_webview must run on the GTK main thread; Tauri commands
        // (set_menu_open, set_webviews_visible, update_content_bounds, etc.)
        // arrive on worker threads, so dispatch via on_main. Calling it
        // directly off-thread silently fails and leaves the old input shape
        // in place — that's why the hamburger dropdown was only clickable in
        // the top-bar strip (the carved hole covered its lower half) and
        // settings overlays could not receive clicks when a real page was
        // visible.
        let chrome_for_shape = chrome.clone();
        crate::maindispatch::on_main(app, move || {
            if capture_all || !web_active {
                let _ = crate::linux_overlay::set_chrome_input_shape(&chrome_for_shape, None);
            } else {
                let _ = crate::linux_overlay::set_chrome_input_shape(
                    &chrome_for_shape,
                    Some((cx as i32, cy as i32, cw as i32, ch as i32)),
                );
            }
        });
    }
}

/// Toggle the chrome "capture all" mode for the hamburger dropdown / in-content
/// chrome popups. While open, the chrome keeps every pointer event (its popup can
/// overlap the content area that is normally click-through to the tab).
pub fn set_menu_open(app: &AppHandle, state: &crate::state::AppState, open: bool) {
    debug_log!("set_menu_open open={open}");
    state.tabs.lock().unwrap().menu_open = open;
    refresh_chrome_input_shape(app, state);
    refresh_tab_input(app, state);
}

/// Keep every tab webview's input region in sync with the current overlay
/// state: blank them while a modal overlay or the hamburger menu is open so
/// pointer events over the content area reach the chrome (KWin tests the tab
/// surfaces first - see `linux_overlay::set_tab_input_shape`), full-surface
/// otherwise. Derived from the flags instead of a caller-provided bool so the
/// menu and overlay command paths can arrive in any order and still settle on
/// the correct state.
fn refresh_tab_input(app: &AppHandle, state: &crate::state::AppState) {
    let (visible, menu_open, handles) = {
        let mgr = state.tabs.lock().unwrap();
        (
            mgr.webviews_visible,
            mgr.menu_open,
            mgr.tabs
                .iter()
                .filter_map(|t| t.webview.clone())
                .collect::<Vec<Webview<Wry>>>(),
        )
    };
    let enabled = visible && !menu_open;
    if handles.is_empty() {
        return;
    }
    // Locks dropped above; with_webview must run on the GTK main thread.
    on_main(app, move || {
        for wv in handles {
            let _ = crate::linux_overlay::set_tab_input_shape(&wv, enabled);
        }
    });
}

fn position_tab(app: &AppHandle, state: &crate::state::AppState, id: u32) -> Result<(), String> {
    let (x, y, w, h) = {
        let mgr = state.tabs.lock().unwrap();
        mgr.content
    };
    position_in_rect(app, state, id, x, y, w, h)
}

fn position_in_rect(
    app: &AppHandle,
    state: &crate::state::AppState,
    id: u32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Result<(), String> {
    // Clone the webview handle out under the lock, then set bounds on main.
    let wv = {
        let mgr = state.tabs.lock().unwrap();
        mgr.get(id).and_then(|t| t.webview.clone())
    };
    if let Some(wv) = wv {
        #[cfg(not(target_os = "linux"))]
        {
            on_main(app, move || -> Result<(), String> {
                // On non-Linux platforms, use absolute positioning
                let _ = wv.set_position(tauri::PhysicalPosition::new(x.round() as i32, y.round() as i32));
                let _ = wv.set_size(tauri::PhysicalSize::new(
                    (w.round() as i32).max(1),
                    (h.round() as i32).max(1),
                ));
                Ok(())
            })?;
        }
        #[cfg(target_os = "linux")]
        {
            // Webview lives in the shared overlay gtk::Fixed (embedded there
            // in realize_tab) - reposition it directly instead of relying on
            // box layout, which never actually moved it.
            on_main(app, {
                let app = app.clone();
                move || -> Result<(), String> {
                    let window = app
                        .get_window("main")
                        .ok_or("main window missing")?;
                    let (tabs_fixed, _chrome_fixed) =
                        crate::linux_overlay::get_or_create_fixeds(&window)?;
                    crate::linux_overlay::reposition(
                        &tabs_fixed,
                        &wv,
                        x.round() as i32,
                        y.round() as i32,
                        w.round() as i32,
                        h.round() as i32,
                    )?;
                    wv.show().ok();
                    Ok(())
                }
            })?;
        }
    }
    Ok(())
}

/// Toggle split view. Turning it on picks the next tab after active as the
/// secondary pane (creating one if none exists); turning it off clears the
/// split and restores the single full-width pane.
pub fn toggle_split(app: &AppHandle, state: &crate::state::AppState) -> Result<bool, String> {
    let was_on = state.tabs.lock().unwrap().split.is_some();
    if was_on {
        let sec = state.tabs.lock().unwrap().split.take();
        if let Some(s) = sec {
            hide_tab(app, state, s);
        }
        reposition_all(app, state);
        emit_tabs(app, state);
        return Ok(false);
    }
    // Off → on: find next tab after active, or create one.
    let (active, existing_next) = {
        let mgr = state.tabs.lock().unwrap();
        let active = mgr.active.ok_or("no active tab")?;
        let idx = mgr
            .tabs
            .iter()
            .position(|t| t.id == active)
            .unwrap_or(0);
        let next = if mgr.tabs.len() >= 2 {
            Some(mgr.tabs[(idx + 1) % mgr.tabs.len()].id)
        } else {
            None
        };
        (active, next)
    };
    let secondary = match existing_next {
        Some(id) => id,
        None => {
            // create_tab activates the new tab; restore the original primary.
            let nid = create_tab(app, state, None);
            state.tabs.lock().unwrap().active = Some(active);
            nid
        }
    };
    state.tabs.lock().unwrap().split = Some(secondary);
    show_and_position(app, state, secondary)?;
    reposition_all(app, state);
    emit_tabs(app, state);
    Ok(true)
}

pub fn set_content_bounds(
    app: &AppHandle,
    state: &crate::state::AppState,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    state.tabs.lock().unwrap().content = (x, y, w, h);
    reposition_all(app, state);
    refresh_chrome_input_shape(app, state);
}

pub fn tab_infos(state: &crate::state::AppState) -> Vec<TabInfo> {
    let mgr = state.tabs.lock().unwrap();
    mgr.tabs
        .iter()
        .map(|t| TabInfo {
            id: t.id,
            title: t.title.clone(),
            url: t.history[t.hist_idx].clone(),
            active: mgr.active == Some(t.id),
            loading: false,
        })
        .collect()
}

pub fn emit_tabs(app: &AppHandle, state: &crate::state::AppState) {
    let tabs = tab_infos(state);
    let (split, cfg) = {
        let mgr = state.tabs.lock().unwrap();
        let cfg = state.config.read().unwrap();
        (mgr.split.is_some(), cfg.clone())
    };
    let payload = serde_json::json!({
        "tabs": tabs,
        "layout": cfg.layout.preset,
        "theme": cfg.theme.name,
        "bookmarksBar": cfg.layout.bookmarks_bar,
        "statusBar": cfg.layout.status_bar,
        "split": split,
    });
    app.emit("umbra://state", payload).ok();
    // Session persistence: every tab mutation flows through here.
    crate::session::save(app);
}
