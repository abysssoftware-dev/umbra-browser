#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use umbra::debug_log;

fn main() {
    let wayland = std::env::var("WAYLAND_DISPLAY")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    // XWayland is only present if Sway actually exported DISPLAY (i.e. XWayland
    // is enabled and spun up). Without it we run native Wayland.
    let has_xwayland = std::env::var("DISPLAY").is_ok();

    // GPU acceleration priority for WebKitGTK on wlroots/Sway:
    //   1. Native Wayland GPU accel (DEFAULT on Wayland) - EGL + DMA-BUF renderer
    //      uses the GPU directly. This is what makes real websites render fast;
    //      the alternative XWayland/GLX path frequently falls back to software
    //      (llvmpipe) under Sway's XWayland, which stutters on heavy pages.
    //   2. XWayland backend - OPT-OUT only via UMBRA_FORCE_XWAYLAND=1. Kept as a
    //      fallback for the rare GPU/driver where native DMA-BUF misbehaves
    //      ("Error 71 (Protocol error) dispatching to Wayland display").
    //   3. Safe software - last resort via UMBRA_DMABUF_WORKAROUND=1.
    // Detect the GPU before boot so we can apply the right workarounds and, on
    // NVIDIA, auto-enable the native Wayland accelerated path (it works there
    // once the explicit-sync negotiation is disabled below).
    let nvidia = std::path::Path::new("/proc/driver/nvidia/version").exists();
    let force_xwayland = std::env::var("UMBRA_FORCE_XWAYLAND").is_ok();

    // Default to native Wayland GPU for any Wayland session. NVIDIA always takes
    // the native path when available.
    if !force_xwayland && wayland && std::env::var("UMBRA_NATIVE_GPU").is_err() {
        std::env::set_var("UMBRA_NATIVE_GPU", "1");
    }
    if nvidia && std::env::var("UMBRA_NATIVE_GPU").is_err() {
        std::env::set_var("UMBRA_NATIVE_GPU", "1");
    }

    let native_gpu_ok = std::env::var("UMBRA_NATIVE_GPU").is_ok();

    // NVIDIA explicit-sync fix for native Wayland GPU: on wlroots + NVIDIA the
    // DMA-BUF renderer's explicit-sync negotiation crashes with "Error 71
    // (Protocol error) dispatching to Wayland display". Disabling the driver's
    // explicit-sync path (a driver-side negotiation issue, not the renderer)
    // keeps the fast accelerated native path alive. Only relevant for NVIDIA;
    // Mesa/AMD/Intel are skipped. Applied automatically when native GPU is on,
    // unless the user already set it explicitly.
    if native_gpu_ok && nvidia && std::env::var("__NV_DISABLE_EXPLICIT_SYNC").is_err() {
        std::env::set_var("__NV_DISABLE_EXPLICIT_SYNC", "1");
    }

    // GPU compositing must stay ON. It drives backdrop-filter blur and smooth
    // scrolling; turning it off forces fully software rendering.
    if std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_err() {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "0");
    }

    if native_gpu_ok {
        // Preferred: native Wayland GPU acceleration via the DMA-BUF renderer.
        if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "0");
        }
        if wayland && std::env::var("GDK_GL").is_err() {
            std::env::set_var("GDK_GL", "egl");
        }
    } else if has_xwayland {
        // Fallback: XWayland backend for GLX-accelerated rendering.
        if std::env::var("GDK_BACKEND").is_err() {
            std::env::set_var("GDK_BACKEND", "x11");
        }
        if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    } else {
        // Native Wayland with no GPU path available: stay safe (no white screen).
        if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        if wayland && std::env::var("GDK_GL").is_err() {
            std::env::set_var("GDK_GL", "egl");
        }
    }

    // Last-resort escape hatch: if GPU compositing misbehaves (or the GBM issue
    // persists), UMBRA_DMABUF_WORKAROUND=1 forces fully software rendering so
    // the app still runs.
    if std::env::var("UMBRA_DMABUF_WORKAROUND").is_ok() {
        std::env::set_var("GDK_GL", "software");
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    debug_log!(
        "gpu setup: wayland={wayland} xwayland={has_xwayland} native_gpu={native_gpu_ok} nvidia={nvidia} nv_explicit_sync={:?} dmabuf_renderer_disabled={} gdk_backend={:?} gdk_gl={:?} compositing={}",
        std::env::var("__NV_DISABLE_EXPLICIT_SYNC").ok(),
        std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").unwrap_or_default(),
        std::env::var("GDK_BACKEND").ok(),
        std::env::var("GDK_GL").ok(),
        std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").unwrap_or_default(),
    );

    // Enable hardware video decoding (VA-API on Linux)
    if std::env::var("GST_VAAPI_ALL_FRAMES").is_err() {
        std::env::set_var("GST_VAAPI_ALL_FRAMES", "1");
    }

    // Fix YouTube/video playback: Enable Media Source Extensions (MSE)
    // and Encrypted Media Extensions (EME) for DRM content
    if std::env::var("WEBKIT_DISABLE_MEDIA_STREAM").is_err() {
        std::env::set_var("WEBKIT_DISABLE_MEDIA_STREAM", "0");
    }

    // Enable Web Audio API for better media support
    if std::env::var("WEBKIT_DISABLE_WEB_AUDIO").is_err() {
        std::env::set_var("WEBKIT_DISABLE_WEB_AUDIO", "0");
    }

    umbra::run()
}

