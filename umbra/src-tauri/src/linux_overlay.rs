//! Linux-only: works around a wry/GTK limitation where child webviews added
//! via `Window::add_child()` land in a `gtk::Box`. wry's `set_bounds()` only
//! calls `size_allocate()` when the webview's parent is a `gtk::Fixed`
//! (see https://github.com/tauri-apps/wry/issues/1727) -- inside a Box, every
//! `set_position()`/`set_size()` call is silently dropped, and Box also
//! reflows/splits space between siblings instead of letting them overlap.
//!
//! Fix: create two `gtk::Fixed` layers packed into the window's default vbox
//! (`Window::default_vbox()`) -- a bottom layer for the tab webviews and a top
//! layer for the chrome webview. Because the chrome layer is packed *after* the
//! tabs layer, it always paints above the tabs without ever re-parenting a
//! webview (re-parenting a WebKitWebView recreates its native surface and drops
//! the alpha/transparency setup, which is what used to blank the page). The
//! chrome layer's content area is made see-through (transparent body + an input
//! shape carve-out) so the page shows through and still receives clicks.
#![cfg(target_os = "linux")]

use gtk::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{window::Window, Runtime};

const ROOT_FIXED_NAME: &str = "umbra-root-fixed";
const TABS_FIXED_NAME: &str = "umbra-tabs-fixed";
const CHROME_FIXED_NAME: &str = "umbra-chrome-fixed";

static EMBED_COUNT: AtomicU64 = AtomicU64::new(0);
static REPOSITION_COUNT: AtomicU64 = AtomicU64::new(0);

fn log_debug(msg: &str) {
    eprintln!("[umbra-overlay] {msg}");
    #[cfg(debug_assertions)]
    crate::debug_log_to_file(&format!("[overlay] {msg}"));
}

/// GTK widgets are main-thread-only, but `with_webview` demands a
/// `Send + 'static` closure. We only ever touch the Fixed from closures
/// dispatched onto the main thread, so moving it across threads inside this
/// wrapper is safe in practice.
struct SendFixed(gtk::Fixed);
unsafe impl Send for SendFixed {}
unsafe impl Sync for SendFixed {}

impl SendFixed {
    fn put(&self, widget: &impl gtk::prelude::IsA<gtk::Widget>, x: i32, y: i32) {
        self.0.put(widget, x, y);
    }

    fn move_(&self, widget: &impl gtk::prelude::IsA<gtk::Widget>, x: i32, y: i32) {
        self.0.move_(widget, x, y);
    }
}

/// Returns the two overlay `gtk::Fixed` layers for `window`: `(tabs, chrome)`.
/// A single root `gtk::Fixed` (which allows overlapping children) is packed into
/// the window's vbox; inside it sit the tabs layer and the chrome layer, both
/// filling the window, with the chrome layer added last so it always paints
/// above the tabs. No webview is ever re-parented, so alpha/transparency is
/// preserved. Creates everything on first call; later calls find the existing
/// widgets by name.
pub fn get_or_create_fixeds<R: Runtime>(
    window: &Window<R>,
) -> Result<(gtk::Fixed, gtk::Fixed), String> {
    let vbox = window
        .default_vbox()
        .map_err(|e| format!("no default gtk vbox on window: {e}"))?;

    // Find or create the root Fixed (the only thing packed in the vbox).
    let root = {
        let mut found: Option<gtk::Fixed> = None;
        for child in vbox.children() {
            if child.widget_name() == ROOT_FIXED_NAME {
                if let Ok(f) = child.downcast::<gtk::Fixed>() {
                    found = Some(f);
                    break;
                }
            }
        }
        match found {
            Some(f) => f,
            None => {
                let f = gtk::Fixed::new();
                f.set_widget_name(ROOT_FIXED_NAME);
                f.show();
                vbox.pack_start(&f, true, true, 0);
                vbox.show_all();
                f
            }
        }
    };

    // Find or create the two nested Fixed layers inside the root.
    let mut tabs_fixed: Option<gtk::Fixed> = None;
    let mut chrome_fixed: Option<gtk::Fixed> = None;
    for child in root.children() {
        if child.widget_name() == TABS_FIXED_NAME {
            if let Ok(f) = child.downcast::<gtk::Fixed>() {
                tabs_fixed = Some(f);
            }
        } else if child.widget_name() == CHROME_FIXED_NAME {
            if let Ok(f) = child.downcast::<gtk::Fixed>() {
                chrome_fixed = Some(f);
            }
        }
    }

    if tabs_fixed.is_none() || chrome_fixed.is_none() {
        // Remove any half-created layer and rebuild both so stacking is correct.
        if let Some(t) = tabs_fixed.take() {
            root.remove(&t);
        }
        if let Some(c) = chrome_fixed.take() {
            root.remove(&c);
        }
        let size = window.inner_size().unwrap_or_default();
        let (w, h) = (size.width.max(1) as i32, size.height.max(1) as i32);
        let tabs = gtk::Fixed::new();
        tabs.set_widget_name(TABS_FIXED_NAME);
        tabs.show();
        tabs.set_size_request(w, h);
        let chrome = gtk::Fixed::new();
        chrome.set_widget_name(CHROME_FIXED_NAME);
        chrome.show();
        chrome.set_size_request(w, h);
        // gtk::Fixed overlaps its children and stacks by add order; the chrome
        // layer is added last so it sits on top of the tabs layer.
        root.put(&tabs, 0, 0);
        root.put(&chrome, 0, 0);
        root.show_all();
        tabs_fixed = Some(tabs);
        chrome_fixed = Some(chrome);
    }

    Ok((tabs_fixed.unwrap(), chrome_fixed.unwrap()))
}

/// Moves `webview`'s underlying GTK widget into `fixed` (if it isn't already
/// there) and places it at the given physical pixel rect. Call this once right
/// after a webview is created.
pub fn embed<R: Runtime>(
    fixed: &gtk::Fixed,
    webview: &tauri::webview::Webview<R>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Result<(), String> {
    let count = EMBED_COUNT.fetch_add(1, Ordering::Relaxed);
    log_debug(&format!("embed #{count}: x={x}, y={y}, w={w}, h={h}"));
    let fixed = SendFixed(fixed.clone());
    webview
        .with_webview(move |pw| {
            let inner = pw.inner();
            let widget: &gtk::Widget = inner.upcast_ref();

            log_debug(&format!(
                "  widget parent before: {:?}",
                widget.parent().map(|p| p.widget_name())
            ));
            if let Some(parent) = widget.parent() {
                if let Ok(old) = parent.downcast::<gtk::Container>() {
                    log_debug(&format!("  removing from parent: {}", old.widget_name()));
                    old.remove(widget);
                }
            }

            // `x/y/w/h` arrive from the frontend as physical pixels ( * devicePixelRatio )
            // but gtk::Fixed coordinates are in logical (CSS) pixels - divide by the
            // widget's scale factor so HiDPI displays don't place the tab 2x too far
            // down/right (which looked like a white screen - the tab was off-screen).
            let scale = widget.scale_factor().max(1) as f64;
            let lx = (x as f64 / scale).round() as i32;
            let ly = (y as f64 / scale).round() as i32;
            let lw = (w as f64 / scale).round() as i32;
            let lh = (h as f64 / scale).round() as i32;
            widget.set_size_request(lw.max(1), lh.max(1));
            fixed.put(widget, lx, ly);
            widget.show();
            log_debug(&format!(
                "  widget parent after: {:?} at {lx},{ly} {lw}x{lh} (phys {x},{y} {w}x{h} scale {scale})",
                widget.parent().map(|p| p.widget_name())
            ));
        })
        .map_err(|e| e.to_string())
}

/// Repositions/resizes a webview that's already inside a Fixed. This is the call
/// that was previously a no-op on Linux.
pub fn reposition<R: Runtime>(
    fixed: &gtk::Fixed,
    webview: &tauri::webview::Webview<R>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Result<(), String> {
    let count = REPOSITION_COUNT.fetch_add(1, Ordering::Relaxed);
    log_debug(&format!("reposition #{count}: x={x}, y={y}, w={w}, h={h}"));
    let fixed = SendFixed(fixed.clone());
    webview
        .with_webview(move |pw| {
            let inner = pw.inner();
            let widget: &gtk::Widget = inner.upcast_ref();
            let scale = widget.scale_factor().max(1) as f64;
            let lx = (x as f64 / scale).round() as i32;
            let ly = (y as f64 / scale).round() as i32;
            let lw = (w as f64 / scale).round() as i32;
            let lh = (h as f64 / scale).round() as i32;
            widget.set_size_request(lw.max(1), lh.max(1));
            fixed.move_(widget, lx, ly);
            widget.queue_resize();
            log_debug(&format!(
                "  widget parent: {:?}, pos {lx},{ly} size: {lw}x{lh} (phys {x},{y} {w}x{h} scale {scale})",
                widget.parent().map(|p| p.widget_name())
            ));
        })
        .map_err(|e| e.to_string())
}

/// Carves the chrome webview's input region so pointer events in `content` pass
/// through to the tab webview below, while the rest of the chrome (top bar,
/// sidebar, overlays) still receives events. `content: None` clears the carve-out
/// (chrome captures everything - used when no real tab is active, e.g. the
/// built-in new-tab page, or while a modal overlay is open).
pub fn set_chrome_input_shape<R: Runtime>(
    webview: &tauri::webview::Webview<R>,
    content: Option<(i32, i32, i32, i32)>,
) -> Result<(), String> {
    webview
        .with_webview(move |pw| {
            let inner = pw.inner();
            let widget: &gtk::Widget = inner.upcast_ref();
            match content {
                None => {
                    log_debug("input_shape: clear (chrome captures all)");
                    widget.input_shape_combine_region(None);
                }
                Some((cx, cy, cw, ch)) => {
                    log_debug(&format!(
                        "input_shape: carve content ({cx},{cy},{cw},{ch})"
                    ));
                    let alloc = widget.allocation();
                    let w = alloc.width();
                    let h = alloc.height();
                    // `content` is in physical pixels (reportBounds * dpr); the input
                    // shape region lives in the widget's CSS-pixel coordinate space, so
                    // convert back by the widget's scale factor.
                    let scale = widget.scale_factor().max(1) as f64;
                    let cx = (cx as f64 / scale) as i32;
                    let cy = (cy as f64 / scale) as i32;
                    let cw = (cw as f64 / scale) as i32;
                    let ch = (ch as f64 / scale) as i32;
                    let region = gtk::cairo::Region::create_rectangle(&gtk::cairo::RectangleInt::new(
                        0, 0, w, h,
                    ));
                    let _ =
                        region.subtract_rectangle(&gtk::cairo::RectangleInt::new(cx, cy, cw, ch));
                    widget.input_shape_combine_region(Some(&region));
                }
            }
        })
        .map_err(|e| e.to_string())
}

/// Toggle a tab webview's input region. The hamburger popup is painted by the
/// chrome, which is stacked *above* the tab webviews, so intuitively it should
/// receive the clicks - but on Wayland/KWin the tab's (sub)surface input region
/// is tested first and wins its rectangle regardless of the parent window's
/// (cleared) shape. Blanking the tab surfaces while the menu is open hands
/// those clicks back to the chrome; `enabled: true` restores the default
/// full-surface region (which is what tabs normally have - only the chrome
/// ever gets an explicit carve).
pub fn set_tab_input_shape<R: Runtime>(
    webview: &tauri::webview::Webview<R>,
    enabled: bool,
) -> Result<(), String> {
    webview
        .with_webview(move |pw| {
            let inner = pw.inner();
            let widget: &gtk::Widget = inner.upcast_ref();
            if enabled {
                log_debug(&format!("tab input: restored ({})", widget.widget_name()));
                widget.input_shape_combine_region(None);
            } else {
                log_debug(&format!("tab input: disabled ({})", widget.widget_name()));
                // A region with no area = the widget receives no pointer events.
                let empty = gtk::cairo::Region::create_rectangles(&[]);
                widget.input_shape_combine_region(Some(&empty));
            }
        })
        .map_err(|e| e.to_string())
}
