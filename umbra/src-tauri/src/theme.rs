use std::collections::BTreeMap;

type Tokens = BTreeMap<String, String>;

fn mk(pairs: &[(&str, &str)]) -> Tokens {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Built-in palettes. Each provides a complete token set per mode so a theme
/// swap is just a config change - every component consumes tokens only.
pub fn builtin_tokens(preset: &str, mode: &str) -> Option<Tokens> {
    let dark = mode == "dark";
    match preset {
        "glass" => Some(glass(dark)),
        "materialyou" | "material" => Some(materialyou(dark)),
        "gruvbox" => Some(gruvbox(dark)),
        "nord" => Some(nord(dark)),
        "catppuccin" => Some(catppuccin(dark)),
        "ocean" => Some(ocean(dark)),
        _ => None,
    }
}

pub fn fallback_tokens() -> Tokens {
    glass(true)
}

/// Shared non-color tokens (geometry, type, motion) merged into every palette.
fn geometry(accent: &str) -> Vec<(&'static str, String)> {
    vec![
        ("radius", "12px".into()),
        ("radius.small", "8px".into()),
        ("spacing", "6px".into()),
        ("chrome.height", "52px".into()),
        ("font.ui", "'Inter', 'Adwaita Sans', system-ui, sans-serif".into()),
        ("font.mono", "'JetBrains Mono', monospace".into()),
        ("blur", "18px".into()),
        ("blur.saturation", "140%".into()),
        ("noise.opacity", if accent == "_light" { "0.03".into() } else { "0.05".into() }),
        ("ease", "cubic-bezier(0.22, 1, 0.36, 1)".into()),
        ("duration", "180ms".into()),
    ]
}

fn finish(extra: Vec<(&'static str, String)>, mut t: Tokens) -> Tokens {
    for (k, v) in extra {
        t.entry(k.into()).or_insert(v);
    }
    t
}

// ── Umbra Glass: glassmorphism, libadwaita-adjacent ──────────────────────────
fn glass(dark: bool) -> Tokens {
    if dark {
        finish(
            geometry(""),
            mk(&[
                ("color.bg", "#1c1c22"),
                ("color.bg.alpha", "0.72"),
                ("color.surface", "#26262e"),
                ("color.fg", "#eeeff2"),
                ("color.fg.dim", "#9a9aa6"),
                ("color.accent", "#8f7bff"),
                ("color.accent.fg", "#ffffff"),
                ("color.border", "#3a3a46"),
                ("color.danger", "#ff6b6b"),
                ("color.ok", "#5fd39a"),
            ]),
        )
    } else {
        finish(
            geometry("_light"),
            mk(&[
                ("color.bg", "#f6f5f4"),
                ("color.bg.alpha", "0.78"),
                ("color.surface", "#ffffff"),
                ("color.fg", "#24252d"),
                ("color.fg.dim", "#6b6b76"),
                ("color.accent", "#6d5ce8"),
                ("color.accent.fg", "#ffffff"),
                ("color.border", "#dcdce2"),
                ("color.danger", "#d64a4a"),
                ("color.ok", "#1f9d61"),
            ]),
        )
    }
}

// ── Material You (Material 3 Purple Palette) ───────────────────────────────
fn materialyou(dark: bool) -> Tokens {
    if dark {
        finish(
            geometry(""),
            mk(&[
                ("color.bg", "#141218"),
                ("color.bg.alpha", "1.0"),
                ("color.surface", "#211f26"),
                ("color.fg", "#e6e0e9"),
                ("color.fg.dim", "#cac4d0"),
                ("color.accent", "#d0bcff"),
                ("color.accent.fg", "#381e72"),
                ("color.border", "#49454f"),
                ("color.danger", "#f2b8b5"),
                ("color.ok", "#b4dfaf"),
                ("radius", "16px"),
                ("radius.small", "10px"),
                ("blur", "0px"),
            ]),
        )
    } else {
        finish(
            geometry("_light"),
            mk(&[
                ("color.bg", "#fef7ff"),
                ("color.bg.alpha", "1.0"),
                ("color.surface", "#f3edf7"),
                ("color.fg", "#1d1b20"),
                ("color.fg.dim", "#49454f"),
                ("color.accent", "#6750a4"),
                ("color.accent.fg", "#ffffff"),
                ("color.border", "#cac4d0"),
                ("color.danger", "#b3261e"),
                ("color.ok", "#2e6c38"),
                ("radius", "16px"),
                ("radius.small", "10px"),
                ("blur", "0px"),
            ]),
        )
    }
}

// ── Gruvbox ──────────────────────────────────────────────────────────────────
fn gruvbox(dark: bool) -> Tokens {
    if dark {
        finish(
            geometry(""),
            mk(&[
                ("color.bg", "#282828"),
                ("color.bg.alpha", "1.0"),
                ("color.surface", "#32302f"),
                ("color.fg", "#ebdbb2"),
                ("color.fg.dim", "#a89984"),
                ("color.accent", "#fe8019"),
                ("color.accent.fg", "#282828"),
                ("color.border", "#504945"),
                ("color.danger", "#fb4934"),
                ("color.ok", "#b8bb26"),
                ("blur", "0px"),
            ]),
        )
    } else {
        finish(
            geometry("_light"),
            mk(&[
                ("color.bg", "#fbf1c7"),
                ("color.bg.alpha", "1.0"),
                ("color.surface", "#f2e5bc"),
                ("color.fg", "#3c3836"),
                ("color.fg.dim", "#7c6f64"),
                ("color.accent", "#d65d0e"),
                ("color.accent.fg", "#fbf1c7"),
                ("color.border", "#d5c4a1"),
                ("color.danger", "#9d0006"),
                ("color.ok", "#79740e"),
                ("blur", "0px"),
            ]),
        )
    }
}

// ── Nord ─────────────────────────────────────────────────────────────────────
fn nord(dark: bool) -> Tokens {
    if dark {
        finish(
            geometry(""),
            mk(&[
                ("color.bg", "#2e3440"),
                ("color.bg.alpha", "1.0"),
                ("color.surface", "#3b4252"),
                ("color.fg", "#eceff4"),
                ("color.fg.dim", "#9ca5b5"),
                ("color.accent", "#88c0d0"),
                ("color.accent.fg", "#2e3440"),
                ("color.border", "#4c566a"),
                ("color.danger", "#bf616a"),
                ("color.ok", "#a3be8c"),
                ("blur", "0px"),
            ]),
        )
    } else {
        finish(
            geometry("_light"),
            mk(&[
                ("color.bg", "#eceff4"),
                ("color.bg.alpha", "1.0"),
                ("color.surface", "#e5e9f0"),
                ("color.fg", "#2e3440"),
                ("color.fg.dim", "#5c6779"),
                ("color.accent", "#5e81ac"),
                ("color.accent.fg", "#eceff4"),
                ("color.border", "#d8dee9"),
                ("color.danger", "#bf616a"),
                ("color.ok", "#a3be8c"),
                ("blur", "0px"),
            ]),
        )
    }
}

// ── Catppuccin (mocha / latte) ───────────────────────────────────────────────
fn catppuccin(dark: bool) -> Tokens {
    if dark {
        finish(
            geometry(""),
            mk(&[
                ("color.bg", "#1e1e2e"),
                ("color.bg.alpha", "1.0"),
                ("color.surface", "#262637"),
                ("color.fg", "#cdd6f4"),
                ("color.fg.dim", "#9399b2"),
                ("color.accent", "#cba6f7"),
                ("color.accent.fg", "#1e1e2e"),
                ("color.border", "#45475a"),
                ("color.danger", "#f38ba8"),
                ("color.ok", "#a6e3a1"),
                ("blur", "0px"),
            ]),
        )
    } else {
        finish(
            geometry("_light"),
            mk(&[
                ("color.bg", "#eff1f5"),
                ("color.bg.alpha", "1.0"),
                ("color.surface", "#e6e9ef"),
                ("color.fg", "#4c4f69"),
                ("color.fg.dim", "#7c7f93"),
                ("color.accent", "#8839ef"),
                ("color.accent.fg", "#eff1f5"),
                ("color.border", "#ccd0da"),
                ("color.danger", "#d20f39"),
                ("color.ok", "#40a02b"),
                ("blur", "0px"),
            ]),
        )
    }
}

// ── Ocean (deep navy + sky-blue accent) ─────────────────────────────────────
fn ocean(dark: bool) -> Tokens {
    if dark {
        finish(
            geometry(""),
            mk(&[
                ("color.bg", "#0d223b"),
                ("color.bg.alpha", "1.0"),
                ("color.surface", "#1a3b5c"),
                ("color.fg", "#e1f2fb"),
                ("color.fg.dim", "#7a9bb3"),
                ("color.accent", "#82c8ea"),
                ("color.accent.fg", "#06283d"),
                ("color.border", "#2a4d6e"),
                ("color.danger", "#ff6b6b"),
                ("color.ok", "#5fd39a"),
                ("blur", "0px"),
            ]),
        )
    } else {
        finish(
            geometry("_light"),
            mk(&[
                ("color.bg", "#e6f0f7"),
                ("color.bg.alpha", "1.0"),
                ("color.surface", "#ffffff"),
                ("color.fg", "#0d223b"),
                ("color.fg.dim", "#46627a"),
                ("color.accent", "#2f8fb8"),
                ("color.accent.fg", "#ffffff"),
                ("color.border", "#b3cfe0"),
                ("color.danger", "#d64a4a"),
                ("color.ok", "#1f9d61"),
                ("blur", "0px"),
            ]),
        )
    }
}

/// Render tokens as CSS custom properties (`--umbra-*`) for the chrome webview.
pub fn css_variables(tokens: &Tokens) -> String {
    let mut out = String::from(":root {\n");
    for (k, v) in tokens {
        let var = k.replace('.', "-");
        out.push_str(&format!("  --umbra-{var}: {v};\n"));
    }
    out.push_str("}\n");
    out
}
