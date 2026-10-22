#!/usr/bin/env bash
# Assembles an AppDir and, when `appimagetool` is available, packs it into an AppImage.
# (ffmpeg is expected on the host; bundling it is left to the packager.)
#   ./packaging/build-appimage.sh
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release
APPDIR=dist/TubeGlass.AppDir
rm -rf "$APPDIR"
install -Dm755 target/release/tubeglass "$APPDIR/usr/bin/tubeglass"
install -Dm644 packaging/tubeglass.desktop "$APPDIR/tubeglass.desktop"
install -Dm644 packaging/tubeglass.desktop "$APPDIR/usr/share/applications/tubeglass.desktop"
install -Dm644 packaging/tubeglass.svg "$APPDIR/tubeglass.svg"
install -Dm644 packaging/tubeglass.svg "$APPDIR/usr/share/icons/hicolor/scalable/apps/tubeglass.svg"
cat > "$APPDIR/AppRun" <<'RUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/tubeglass" "$@"
RUN
chmod 755 "$APPDIR/AppRun"

if command -v appimagetool >/dev/null 2>&1; then
  ARCH=$(uname -m) appimagetool "$APPDIR" "dist/TubeGlass-$(uname -m).AppImage"
  echo "Built dist/TubeGlass-$(uname -m).AppImage"
else
  echo "AppDir ready in $APPDIR — install appimagetool to pack it into an .AppImage."
fi
