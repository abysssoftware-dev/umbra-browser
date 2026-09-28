# Umbra — Setup & Configuration Guide

Umbra is a fully customizable browser built on **Tauri v2** (Rust backend, WebKitGTK rendering). It is its own browser — not a Firefox, LibreWolf, or Chromium fork; there is no Gecko or Blink code in the tree. Strong defaults out of the box — but every part of the chrome can be torn down and rebuilt via TOML config, themes, and sandboxed Lua/WASM plugins.

---

## 1. Build & Run

### Prerequisites

- **Rust toolchain** (via [rustup](https://rustup.rs))
- **Node 18+ / npm**
- **Linux system deps** (Debian/Ubuntu) — Tauri needs WebKitGTK and friends:

```sh
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

### Build

```sh
# frontend deps
npm --prefix umbra/ui install

# release build of the Rust backend
cd umbra/src-tauri
cargo build --release
```

For development, the Tauri CLI gives you hot-reloading dev mode:

```sh
npm install -g @tauri-apps/cli   # or use cargo install tauri-cli
cd umbra/src-tauri
cargo tauri dev      # dev build with live reload
cargo tauri build    # production bundle
```

### The `--safe` flag

Launch with `umbra --safe` (or `cargo run -- --safe`) to boot in **safe mode**: all plugins and user config are disabled, and the previous session is not restored. This is your recovery path if a broken config or plugin prevents normal startup — like a GNOME fallback session for your browser. You can verify safe mode from the control socket (`safe` command).

---

## 2. Configuration

Umbra's config lives at:

```
~/.config/umbra/umbra.toml
```

The file is created with defaults on first run. The Settings GUI (open with `Ctrl+,`) writes this same file under the hood — power users can hand-edit it afterwards.

### Annotated example

```toml
version = 1                      # config schema version
plugins = []                     # plugin directory names under ~/.config/umbra/plugins/, loaded in order
user_agent = ""                  # override the browser UA; empty = built-in desktop-Firefox-lookalike
                                 # (cosmetic only; UMBRA_USER_AGENT env still wins when set)

[theme]
name = "umbra-glass"             # installed theme looked up under themes/<name>/tokens.toml
preset = "glass"                 # built-in palette: glass | material | gruvbox | nord | catppuccin | ocean | custom
mode = "dark"                    # dark | light
tokens = { "color.accent" = "#8f7bff", "radius" = "12px" }   # custom token overrides

[layout]
preset = "compact"               # compact | minimal | sidebar | zen
bookmarks_bar = true
status_bar = true
sidebar_width = 220              # px, sidebar layout only

[keybindings]                     # overrides layered on top of the built-in defaults
"Ctrl+K" = "palette"              # any KNOWN_ACTIONS target is remappable;
"Ctrl+T" = "new_tab"              # delete a chord here to fall back to the default

[flags]
adblock = true                   # built-in ad/tracker blocker (default-on, removable)
pip = true                       # picture-in-picture
split_view = true                # split view
modal_mode = false               # vim-style modal command mode
contextual_hints = true          # contextual usage tips
notify_reload = false            # toast a notification whenever the config reloads

[newtab]                         # built-in new-tab page (umbra://newtab)
url = ""                         # any URL replaces the built-in page entirely when set
background = ""                  # image URL or absolute file path (empty = default gradient)
greeting = ""                    # line shown under the clock
show_clock = true                # show the clock on the built-in page
```

### Validation & auto-rollback

Every config load runs through schema validation (unknown layout presets/themes/actions are rejected with a clear error, never silently applied). On every **successful** reload Umbra snapshots the config to `~/.config/umbra/snapshots/last-good.toml`. If a reload fails validation, Umbra automatically rolls back to that snapshot and shows a toast — a broken edit never takes the browser down.

### Hot reload

A filesystem watcher monitors the config directory. Save `umbra.toml` (or install a theme/package) and changes apply live — no restart required.

---

## 3. Theming

Themes work in three layers: design tokens → component skins → full component replacement. All components consume design tokens only; nothing is hardcoded.

### Design tokens

| Token | Meaning | Default (glass dark) |
|---|---|---|
| `color.bg` | Base background | `#1c1c22` |
| `color.bg.alpha` | Chrome background alpha (glass effect) | `0.72` |
| `color.surface` | Raised surface color | `#26262e` |
| `color.fg` | Foreground text | `#eeeff2` |
| `color.fg.dim` | Dimmed text | `#9a9aa6` |
| `color.accent` | Accent color (buttons, highlights) | `#8f7bff` |
| `color.accent.fg` | Text on accent | `#ffffff` |
| `color.border` | Border color | `#3a3a46` |
| `color.danger` | Error/danger color | `#ff6b6b` |
| `color.ok` | Success color | `#5fd39a` |
| `radius` | Corner radius | `12px` |
| `radius.small` | Small-element radius | `8px` |
| `spacing` | Base spacing unit | `6px` |
| `chrome.height` | Top bar height | `52px` |
| `blur` | Backdrop blur amount | `18px` |
| `blur.saturation` | Backdrop saturation | `140%` |
| `noise.opacity` | Grain texture opacity over blur | `0.05` |
| `font.ui` | UI font stack | Inter / Adwaita Sans / system-ui |
| `font.mono` | Monospace font stack | JetBrains Mono |
| `ease` | Motion easing curve | `cubic-bezier(0.22, 1, 0.36, 1)` |
| `duration` | Transition duration | `180ms` |

Tokens are exposed to the chrome as CSS custom properties (`--umbra-color-bg`, `--umbra-radius`, …).

### Overriding tokens

Two ways:

1. **Config**: add entries under `[theme] tokens` in `umbra.toml`, or set them live from Settings → *Appearance* (Quick customization / Custom Theme) or the command palette (`token color.accent #ff0055`). Empty value removes an override.
2. **Installed theme**: drop a theme into `~/.config/umbra/themes/<name>/tokens.toml` and set `[theme] name = "<name>"`.

Required tokens (`color.bg`, `color.fg`, `color.accent`, `radius`, `blur`) must resolve after palette + overrides, or validation rejects the config.

### `.umbrapkg` packages

Themes and plugins distribute as `.umbrapkg` files — zip archives containing at minimum a `manifest.toml`:

```toml
name = "my-theme"
version = "1.0.0"
type = "theme"            # theme | plugin | layout
interface_version = 1     # targeted slot-interface version

[capabilities]            # plugin capability flags (themes usually omit)
fs = false
net = false
ui = true
```

Plus optional contents: `tokens.toml` (theme token overrides), `components/` (replacement components), `scripts/` (Lua), `assets/`.

**Installing:** open Settings → *Plugins* → *Packages & Gallery* → either browse the community gallery (GitHub topics `umbra-theme` / `umbra-plugin`) or paste a GitHub URL / local path to a `.umbrapkg`. Packages are staged first — a diff view shows exactly what tokens/components/capabilities would be applied before you confirm.

---

## 4. Plugins

### Directory layout

```
~/.config/umbra/plugins/
└── my-plugin/
    ├── manifest.toml
    └── main.lua
```

Example `manifest.toml`:

```toml
name = "my-plugin"
version = "0.1.0"
type = "plugin"
entry = "main.lua"
interface_version = 1

[capabilities]
fs = false
net = false
ui = true
```

Enable it by adding `"my-plugin"` to the `plugins` list in `umbra.toml`.

**Capability flags:** `fs` (filesystem access), `net` (network requests), `ui` (component registration). Nothing outside the declared capabilities is granted — no filesystem/network access unless explicitly given.

### Lua API (`umbra.*`)

```lua
-- Mount one of your components into a slot.
-- Core slots: top-bar, tab-strip, bookmarks-bar, sidebar-left,
-- sidebar-right, status-bar, overlay (custom slots also allowed).
umbra.register_component{ component = "pomodoro", slot = "status-bar" }

umbra.log("hello from my-plugin")

-- Only present when net = true is granted:
local res = umbra.net.request{ url = "https://example.com", method = "GET" }
-- res.status, res.body

umbra.interface_version  -- current slot interface version constant
```

Plugins may define a global `init()` which runs after the script loads.

### Resource caps

Each plugin runs in its own sandboxed Lua VM with a **32 MiB memory cap** and an instruction-counting CPU budget (**50M VM instructions**) — a runaway plugin errors out instead of pegging a core.

### WASM tier (advanced)

Plugins whose manifest `entry` ends in `.wasm` run via wasmtime instead of Lua:

- Module must export `umbra_main()` (or `_start` for WASI-style modules).
- Always linked host functions: `umbra_log(ptr, len)` — writes to the plugin console.
- Linked only when `net = true`: `umbra_net_request(url_ptr, url_len, out_ptr) -> i32` — GET with a 10s timeout and a hard 5 MiB response cap.
- `fs = true` is accepted but WASI is not wired up yet: such modules load with a warning and are skipped.
- Same manifest, diff-view, and gallery pipeline as Lua plugins; fuel-based CPU budget (100M fuel units) where available.

---

## 5. Control Socket

Umbra exposes a Unix socket at `~/.config/umbra/umbra.sock` — script the browser externally, `hyprctl`-style. One command per line:

```sh
SOCK=~/.config/umbra/umbra.sock

echo "ping" | socat - UNIX-CONNECT:$SOCK        # → pong
echo "tabs" | socat - UNIX-CONNECT:$SOCK        # JSON list of tabs
echo "open https://example.com" | socat - UNIX-CONNECT:$SOCK   # navigate active tab
echo "new https://example.com" | socat - UNIX-CONNECT:$SOCK    # new tab with URL
echo "reload" | socat - UNIX-CONNECT:$SOCK      # reload config + chrome
echo "layout sidebar" | socat - UNIX-CONNECT:$SOCK
echo "slots" | socat - UNIX-CONNECT:$SOCK       # mounted components per slot
echo "plugins" | socat - UNIX-CONNECT:$SOCK     # loaded plugins + status
echo "bm" | socat - UNIX-CONNECT:$SOCK          # bookmarks as JSON
echo "safe" | socat - UNIX-CONNECT:$SOCK        # safe-mode state
echo "quit" | socat - UNIX-CONNECT:$SOCK        # save session & exit
```

(`help` lists all commands: `ping`, `tabs`, `open <url>`, `new <url>`, `reload`, `layout <preset>`, `slots`, `plugins`, `bm`, `safe`, `quit`.)

---

## 6. Browser Import

On first launch (or any time after, from **Settings → Import**) Umbra scans the default XDG paths for installed browser profiles and imports **bookmarks and history**:

- **Chromium family:** Chrome, Chromium, Brave (stable/beta/dev/nightly), Edge, Vivaldi, Opera, Arc, Yandex, Epiphany.
- **Firefox family:** Firefox, LibreWolf, Zen, Waterfox, Floorp, Ghostery, SeaMonkey, Basilisk, Pale Moon.

Detection is silent — missing browsers are simply omitted. Passwords are intentionally **not** imported: there is no vault in Umbra, and plaintext password import is unsafe by design.

Importing *from* Firefox or LibreWolf is not the same as being built on them: Umbra renders through WebKitGTK and shares no code with those browsers.

## 7. Config & Bookmark Sync

Sync pushes/pulls `umbra.toml` + `bookmarks.toml` to a private GitHub gist you configure with a personal-access token (stored with `0600` perms). Browsing data (history/tabs) is never synced — config and bookmarks only.

## 8. Split View & PiP

- **Split view:** `Ctrl+\` splits the content area between the active tab and the next tab.
- **Picture-in-picture:** `Ctrl+Shift+P` toggles a small frameless always-on-top window mirroring the active tab URL.
- **Fullscreen:** `F11`.

## 9. New Tab Page & User-Agent

- **New tab:** the built-in page is `umbra://newtab` (clock, optional greeting, optional background image). Set `[newtab] url` in `umbra.toml` to open any URL — or a custom start page — instead.
- **User-Agent:** Umbra's built-in UA presents as desktop Firefox on Linux (a cosmetic choice so sites don't flag it as an unknown mobile Safari; no Gecko is involved). Presets and a free-text override live in **Settings → Browser** and persist to `user_agent` in `umbra.toml`. The `UMBRA_USER_AGENT` environment variable wins over both, and applies to new tabs/navigations.

## 10. Troubleshooting

| Symptom | Fix |
|---|---|
| **Blank window on launch** | Missing WebKitGTK dependencies — install the Linux system packages listed in section 1. |
| **Config broke, browser misbehaving** | Malformed config is rejected automatically; Umbra rolls back to the last-known-good snapshot and shows a toast. To start clean, fix or delete `~/.config/umbra/umbra.toml` (defaults regenerate on next launch), or launch with `--safe`. |
| **Plugin errors** | Open the plugin console panel (`Ctrl+Shift+C`) — logs, load warnings, and per-plugin status/errors surface there. |
| **Hot reload didn't fire** | Enable `notify_reload` in `[flags]` to get a toast on every config reload, so you can tell whether the watcher picked the change up. |
| **Blocked site after enabling adblock** | The built-in blocker matches a host blocklist (exact or subdomain). Turn it off with `adblock = false` in `[flags]`, or report a false positive. |
