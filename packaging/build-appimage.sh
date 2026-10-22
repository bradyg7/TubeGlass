#!/usr/bin/env bash
# Assembles an AppDir and, when `appimagetool` is available, packs it into
# dist/TubeGlass-<version>-linux-<arch>.AppImage.
# (ffmpeg is expected on the host; bundling it is left to the packager.)
#   ./packaging/build-appimage.sh
# Set SKIP_BUILD=1 to package an already built target/release/tubeglass.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
ARCH=$(uname -m)
[[ -n "${SKIP_BUILD:-}" ]] || cargo build --release --locked
APPDIR=dist/TubeGlass.AppDir
rm -rf "$APPDIR"
install -Dm755 target/release/tubeglass "$APPDIR/usr/bin/tubeglass"
install -Dm644 packaging/tubeglass.desktop "$APPDIR/tubeglass.desktop"
install -Dm644 packaging/tubeglass.desktop "$APPDIR/usr/share/applications/tubeglass.desktop"
install -Dm644 packaging/tubeglass.svg "$APPDIR/tubeglass.svg"
install -Dm644 packaging/tubeglass.png "$APPDIR/tubeglass.png"
install -Dm644 packaging/tubeglass.svg "$APPDIR/usr/share/icons/hicolor/scalable/apps/tubeglass.svg"
ln -sf tubeglass.png "$APPDIR/.DirIcon"
cat > "$APPDIR/AppRun" <<'RUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/tubeglass" "$@"
RUN
chmod 755 "$APPDIR/AppRun"

OUT="dist/TubeGlass-${VERSION}-linux-${ARCH}.AppImage"
if command -v appimagetool >/dev/null 2>&1; then
  ARCH="$ARCH" appimagetool "$APPDIR" "$OUT"
  echo "Built $OUT"
else
  echo "AppDir ready in $APPDIR — install appimagetool to pack it into an .AppImage."
fi
