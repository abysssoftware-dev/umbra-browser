use tauri::Emitter;

/// Lightweight built-in ad/tracker blocker (spec: "default-on but removable").
/// Static embedded list - exact or subdomain-of match on the URL host.
pub const BLOCKED_DOMAINS: &[&str] = &[
    // ── Google / DoubleClick ──
    "doubleclick.net", "googlesyndication.com", "googleadservices.com",
    "google-analytics.com", "googletagmanager.com", "googletagservices.com",
    "adservice.google.com", "pagead2.googlesyndication.com", "admob.com",
    "ads.google.com", "googlecommerce.com", "crashlytics.com",
    // ── Facebook / Meta ──
    "connect.facebook.net", "graph.facebook.com", "facebook.net",
    "fbcdn.net", "atdmt.com", "atlassbx.com", "friendfeed.com",
    "instagram.com/plugin", "lookaside.fbsbx.com",
    // ── Amazon ──
    "amazon-adsystem.com", "cloudfront-labs.amazonaws.com",
    "aax.amazon-adsystem.com", "s.amazon-adsystem.com",
    // ── Major ad networks ──
    "adnxs.com", "adsrvr.org", "rubiconproject.com", "pubmatic.com",
    "openx.net", "criteo.com", "criteo.net", "casalemedia.com",
    "smartadserver.com", "adform.net", "yieldmo.com", "indexexchange.com",
    "contextweb.com", "gumgum.com", "sovrn.com", "lijit.com",
    "triplelift.com", "sharethrough.com", "spotxchange.com",
    "tremorhub.com", "teads.tv", "outbrain.com", "taboola.com",
    "zergnet.com", "revcontent.com", "mgid.com", "content-ad.net",
    "adblade.com", "adcolony.com", "applovin.com", "chartboost.com",
    "inmobi.com", "mopub.com", "unityads.unity3d.com", "vungle.com",
    "ironsrc.com", "supersonic.com", "aerserv.com", "adtech.de",
    "aol-cloud.com", "advertising.com", "adtechus.com", "adtechjp.com",
    // ── Exchanges / DSPs ──
    "bidswitch.net", "districtm.io", "improvedigital.com",
    "adition.com", "adsafeprotected.com", "adsnative.com",
    "adroll.com", "adrolr.com", "sitescout.com", "turn.com",
    "yahoo.co.jp/ad", "bluekai.com", "agkn.com", "krxd.net",
    "exelator.com", "mathtag.com", "rlcdn.com", "tapad.com",
    "crwdcntrl.net", "demdex.net", "everesttech.net", "omtrdc.net",
    "2mdn.net", "3lift.com", "am15.net", "bidr.io", "buysellads.com",
    // ── Analytics / telemetry ──
    "hotjar.com", "hotjar.io", "mouseflow.com", "fullstory.com",
    "crazyegg.com", "clicktale.net", "quantserve.com", "scorecardresearch.com",
    "comscore.com", "newrelic.com", "nr-data.net", "bugsnag.com",
    "mixpanel.com", "segment.io", "segment.com", "amplitude.com",
    "heapanalytics.com", "kissmetrics.com", "kissmetrics.io",
    "mousestats.com", "inspectlet.com", "luckyorange.com",
    "statcounter.com", "histats.com", "clicky.com", "getclicky.com",
    "chartbeat.com", "parsely.com", "parse.ly", "branch.io",
    "appsflyer.com", "adjust.com", "kochava.com", "tenjin.io",
    // ── Tag managers / A-B testing ──
    "optimizely.com", "crazyegg.com", "vwo.com", "visualwebsiteoptimizer.com",
    "abtasty.com", "convert.com", "maxymiser.com", "monetate.net",
    // ── Privacy-hostile trackers ──
    "scorecardresearch.com", "quantcast.com", "quantcount.com",
    "moatads.com", "moatpixel.com", "sentry-cdn.com", "getsentry.com",
    "intercom.io", "widget.intercom.io", "hotmart.com", "onesignal.com",
    "pushwoosh.com", "pushcrew.com", "truepush.com", "sendpulse.com",
    "mailmunch.co", "sumo.com", "hellobar.com", "optinmonster.com",
    // ── Taboola-adjacent / popups ──
    "popads.net", "popcash.net", "propellerads.com", "propellerclick.com",
    "adcash.com", "onclickalgo.com", "onclickmega.com", "onclickperformance.com",
    "adsterra.com", "adsterratech.com", "hilltopads.net", "clickadu.com",
    "exoclick.com", "exosrv.com", "juicyads.com", "trafficjunky.net",
    // ── Misc common ──
    "zedo.com", "adbrite.com", "adknowledge.com", "adyard.de",
    "adzerk.net", "adzerk.com", "longtailvideo.com", "jwpcdn.com",
    "brightcove.com", "theplatform.com", "freewheel.tv", "fwmrm.net",
    "spot.im", "disqusads.com", "ads.yahoo.com", "admarketplace.net",
    "7search.com", "media.net", "medianet.com", "yadro.ru", "top100.ru",
];

/// True if `host` equals a blocked domain or is a subdomain of one.
pub fn is_blocked_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    BLOCKED_DOMAINS
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

/// Extract the host component from a URL string (no url crate - hand-rolled).
pub fn extract_host(url: &str) -> Option<String> {
    let rest = url.split("://").nth(1)?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?; // drop userinfo
    let host = host.split(':').next()?; // drop port
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

/// Check the active-tab navigation against the blocklist when the `adblock`
/// flag is on. Rejects with an error and notes it in the plugin console.
pub fn check_navigation(
    app: &tauri::AppHandle,
    state: &crate::state::AppState,
    url: &str,
) -> Result<(), String> {
    let enabled = {
        let cfg = state.config.read().unwrap();
        *cfg.flags.get("adblock").unwrap_or(&true)
    };
    if !enabled {
        return Ok(());
    }
    let Some(host) = extract_host(url) else {
        return Ok(()); // umbra:// pages, file paths etc. pass through
    };
    if is_blocked_host(&host) {
        app.emit(
            "umbra://console",
            serde_json::json!({"level": "warn", "message": format!("adblock: blocked {host}")}),
        )
        .ok();
        return Err(format!("blocked by umbra adblock: {host}"));
    }
    Ok(())
}
