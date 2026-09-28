#!/usr/bin/env bash
# Portable Windows build, cross-compiled from Linux with mingw-w64.
#
#   ./scripts/build-windows-cross.sh
#
# Output lands in dist/:
#   umbra-<version>-windows-x86_64.zip   (umbra.exe + README + LICENSE)
#   SHA256SUMS                           (refreshed for everything in dist/)
#
# Prerequisites (both must be on PATH / installed via rustup):
#   rustup target add x86_64-pc-windows-gnu
#   mingw-w64 gcc (x86_64-w64-mingw32-gcc)
#
# NOTES
#   * This only ever yields a portable .exe. NSIS/MSI installers cannot be
#     built on Linux - they come from the windows job in
#     .github/workflows/release.yml.
#   * The portable exe expects the WebView2 Evergreen runtime on the target
#     machine (preinstalled on current Windows 10/11; the installers bundle
#     the download bootstrapper, the zip does not).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONF="$ROOT/umbra/src-tauri/tauri.conf.json"
DIST="$ROOT/dist"
UI="$ROOT/umbra/ui"
TARGET=x86_64-pc-windows-gnu
EXE="$ROOT/umbra/src-tauri/target/$TARGET/release/umbra.exe"

VERSION="$(node -p "require('$CONF').version")"

die() { echo "error: $*" >&2; exit 1; }

# ---------------------------------------------------------------- checks --
command -v cargo >/dev/null 2>&1 || die "cargo not found (install rustup)"
command -v node >/dev/null 2>&1 || die "node not found"
command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1 ||
  die "x86_64-w64-mingw32-gcc not found (install mingw-w64)"
rustup target list --installed 2>/dev/null | grep -qx "$TARGET" ||
  die "rust target $TARGET missing - run: rustup target add $TARGET"

# -------------------------------------------------------------- frontend --
# tauri-build embeds ../ui/build at compile time; cargo build does NOT run
# tauri's beforeBuildCommand, so build the frontend here.
[ -d "$UI/node_modules" ] || npm --prefix "$UI" install
echo "==> Building frontend"
npm --prefix "$UI" run build

# ------------------------------------------------------------------ rust --
echo "==> Cross-compiling umbra $VERSION ($TARGET)"
cargo build --release --target "$TARGET" --manifest-path "$ROOT/umbra/src-tauri/Cargo.toml"

[ -f "$EXE" ] || die "expected binary not found at $EXE"

# ------------------------------------------------------------------ zip ---
NAME="umbra-$VERSION-windows-x86_64"
echo "==> Packaging $NAME.zip"
# Leave sibling artifacts (deb/rpm/tarball) alone so both build scripts can
# share one dist/ and one SHA256SUMS.
mkdir -p "$DIST"
rm -f "$DIST/$NAME.zip"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

mkdir -p "$STAGE/$NAME"
cp "$EXE" "$STAGE/$NAME/umbra.exe"
cp "$ROOT/README.md" "$STAGE/$NAME/README.md"
cp "$ROOT/LICENSE" "$STAGE/$NAME/LICENSE"

(cd "$STAGE" && zip -qr "$DIST/$NAME.zip" "$NAME")

# ------------------------------------------------------------- checksums --
# Cover whatever is currently in dist/ so linux + windows artifacts share one
# SHA256SUMS file when both scripts have run.
echo "==> Checksums"
(
  cd "$DIST"
  find . -maxdepth 1 -type f \
    \( -name '*.deb' -o -name '*.rpm' -o -name '*.tar.gz' -o -name '*.zip' \) \
    -printf '%f\n' | sort | xargs -r sha256sum > SHA256SUMS
)

echo
echo "Artifacts:"
ls -lh "$DIST"
