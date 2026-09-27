#!/usr/bin/env bash
# Production build for Linux: deb + rpm + portable tarball, all checksummed.
#
#   ./scripts/build-linux.sh
#
# Outputs land in dist/:
#   umbra_<version>_<arch>.deb
#   umbra-<version>-<release>.<arch>.rpm
#   umbra-<version>-linux-<arch>.tar.gz
#   SHA256SUMS
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONF="$ROOT/umbra/src-tauri/tauri.conf.json"
DIST="$ROOT/dist"
RELEASE_BIN="$ROOT/umbra/src-tauri/target/release/umbra"

VERSION="$(node -p "require('$CONF').version")"
ARCH="$(uname -m)"

# Locate the Tauri CLI: PATH first, then npx.
if command -v tauri >/dev/null 2>&1; then
  TAURI=(tauri)
elif [ -x "$ROOT/umbra/ui/node_modules/.bin/tauri" ]; then
  TAURI=("$ROOT/umbra/ui/node_modules/.bin/tauri")
else
  TAURI=(npx --yes @tauri-apps/cli@2)
fi

echo "==> Building Umbra $VERSION (deb, rpm)"
cd "$ROOT/umbra/src-tauri"
"${TAURI[@]}" build

[ -x "$RELEASE_BIN" ] || { echo "error: release binary not found at $RELEASE_BIN" >&2; exit 1; }

echo "==> Staging artifacts in $DIST"
rm -rf "$DIST"
mkdir -p "$DIST"

find "$ROOT/umbra/src-tauri/target/release/bundle/deb" -maxdepth 1 -name '*.deb' -exec cp {} "$DIST/" \;
find "$ROOT/umbra/src-tauri/target/release/bundle/rpm" -maxdepth 1 -name '*.rpm' -exec cp {} "$DIST/" \;

echo "==> Packaging tarball umbra-$VERSION-linux-$ARCH.tar.gz"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

mkdir -p "$STAGE/umbra-$VERSION-linux-$ARCH"
cp "$RELEASE_BIN" "$STAGE/umbra-$VERSION-linux-$ARCH/umbra"
cp "$ROOT/README.md" "$STAGE/umbra-$VERSION-linux-$ARCH/README.md"

cat > "$STAGE/umbra-$VERSION-linux-$ARCH/umbra.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Umbra
GenericName=Web Browser
Comment=Browse without compromise
Exec=umbra %u
Icon=umbra
Terminal=false
Categories=Network;WebBrowser;
StartupNotify=true
EOF

tar -C "$STAGE" -czf "$DIST/umbra-$VERSION-linux-$ARCH.tar.gz" "umbra-$VERSION-linux-$ARCH"

echo "==> Checksums"
(
  cd "$DIST"
  sha256sum ./*.deb ./*.rpm ./*.tar.gz > SHA256SUMS
)

echo
echo "Artifacts:"
ls -lh "$DIST"
