# Umbra Browser

Browse without compromise. Umbra is a fully customizable, dotfiles-first browser built on **Rust + Tauri v2**. Configure it like your window manager: every piece of chrome mounts into slots you can tear down and rebuild, one live TOML file drives it all, themes and plugins install from the community, and nothing ever phones home.

**[Documentation](https://github.com/abysssoftware-dev/umbra-browser)** · **[Repository](https://github.com/abysssoftware-dev/umbra-browser)**

---

## Core Philosophy

* **Tear It Down, Rebuild It:** Tab bar, URL bar, bookmarks bar, sidebar, status bar: every component mounts into named slots (`top-bar`, `tab-strip`, `bookmarks-bar`, `sidebar-left`, `status-bar`, `overlay`). Remove a component and it is never instantiated. Nothing about the chrome is sacred.
* **One TOML to Rule Them All:** Layout, theme, keybindings, flags: everything lives in one file at `~/.config/umbra/umbra.toml`. Save it in any editor and changes apply live with zero restart. If you make a syntax error, Umbra rolls back to the last-known-good snapshot automatically.
* **Themes from Tokens to Total Rebuilds:** Built-in presets include **Glass** (default signature purple), **Material You** (M3 expressive purple), **Gruvbox**, **Nord**, **Catppuccin**, and **Rosé Pine**. A token file swap reskins everything; full component replacement goes all the way to boxy TUI styles.
* **Sandboxed Plugins:** Lua and WASM plugins gated by capability manifests (`fs`, `net`, `ai`, `ui`). No filesystem or network access is granted unless explicitly declared. Per-plugin memory and CPU budgets keep your machine fast.
* **AI That Lives on Your Machine:** Optional page summarization, reading assistant Q&A, and natural-language tab search via your local Ollama instance (`127.0.0.1:11434`). It stays off until you enable it, and when enabled, zero data ever leaves your machine. No API keys, no telemetry, no cloud.
* **Private by Architecture:** Built-in ad and tracker blocker enabled by default (and removable, like everything else). No telemetry, no pings, no cloud calls anywhere in the browser.

---

## Architecture

| Layer | Technology |
|---|---|
| **Shell & UI** | SvelteKit + Tailwind CSS + GSAP (slot-based, reactive) |
| **Backend Core** | Rust + Tauri v2 |
| **Render Engine** | WebKitGTK (`webkit2gtk-4.1`) |
| **Plugins** | Lua (`mlua`) + WASM (`wasmtime`) with capability security manifests |
| **Local AI** | Ollama integration (strictly `127.0.0.1:11434`) |
| **Config Engine** | Validated TOML with automatic rollback snapshots (`umbra.toml`) |
| **Distribution** | `.umbrapkg` packages with GitHub topic-based community gallery |

---

## Layout Presets

Umbra comes with multiple built-in layout engines:
* **Compact:** Clean, minimal top-bar with tab strip and URL pill (default).
* **Sidebar:** Vertical tab strip in a collapsible glass sidebar.
* **Minimal:** Focused view with centered address pill and hidden navigation buttons.
* **Tiling:** Paned grid with side-by-side split-view support (`Ctrl+\`).
* **Floating:** Playful stacked and tilted tab cards.
* **Zen:** Chrome fades out completely until hover or focus.

---

## Building from Source

Building Umbra requires only **Rust** and **Node.js**; no massive Chromium/Firefox compilation overhead.

### Prerequisites

* Rust (stable, 1.77+)
* Node.js (18+) & npm
* WebKitGTK build dependencies on Linux:
  ```bash
  sudo apt install libwebkit2gtk-4.1-dev build-essential libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

### Build Steps

```bash
# Clone the repository
git clone https://github.com/abysssoftware-dev/umbra-browser.git
cd umbra-browser

# Install frontend dependencies
npm --prefix umbra/ui install

# Run in development mode (with hot reload and DevTools)
cd umbra/src-tauri
cargo tauri dev

# Build optimized production release
cargo tauri build
```

The compiled binary and packages will be located in `umbra/src-tauri/target/release/bundle/`.

For a packaged Linux release (`.deb` + `.rpm` + portable tarball + `SHA256SUMS`, all staged in `dist/`):

```bash
./scripts/build-linux.sh
```

To cross-compile a portable Windows build from Linux (requires the `x86_64-pc-windows-gnu` rust target and mingw-w64):

```bash
./scripts/build-windows-cross.sh   # -> dist/umbra-<version>-windows-x86_64.zip
```

NSIS/MSI installers can only be produced on Windows — the `windows` job in `.github/workflows/release.yml` builds them on every tag push.

---

## Keybindings

| Shortcut | Action |
|---|---|
| `Ctrl+T` | New tab |
| `Ctrl+W` | Close tab |
| `Ctrl+L` / `Ctrl+E` | Focus address bar |
| `Ctrl+R` / `F5` | Reload page |
| `Ctrl+D` | Bookmark current page |
| `Ctrl+K` | Command palette |
| `Ctrl+,` | Open Settings |
| `Ctrl+\` | Toggle side-by-side split view |
| `Ctrl+Shift+D` | Toggle dark / light mode |
| `Ctrl+Alt+L` | Cycle through layouts |
| `Ctrl+Shift+C` | Toggle plugin console |
| `Alt+←` / `Alt+→` | Back / Forward navigation |
| `F11` | Toggle fullscreen |
| `Esc` | Close dialogs and overlays |

---

## License

GPL-3.0. See [LICENSE](LICENSE) for details.
