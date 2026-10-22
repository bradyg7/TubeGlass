#!/usr/bin/env bash
# Builds tubeglass_<version>_<arch>.deb without needing dpkg (only ar + tar).
#   ./packaging/build-deb.sh            -> dist/tubeglass_0.1.0_amd64.deb
# Set SKIP_BUILD=1 to package an already built target/release/tubeglass.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
case "$(uname -m)" in
  x86_64) ARCH=amd64 ;;
  aarch64) ARCH=arm64 ;;
  *) ARCH=$(uname -m) ;;
esac

[[ -n "${SKIP_BUILD:-}" ]] || cargo build --release --locked
ROOT=$(mktemp -d)
trap 'rm -rf "$ROOT"' EXIT

install -Dm755 target/release/tubeglass "$ROOT/data/usr/bin/tubeglass"
install -Dm644 packaging/tubeglass.desktop "$ROOT/data/usr/share/applications/tubeglass.desktop"
install -Dm644 packaging/tubeglass.svg "$ROOT/data/usr/share/icons/hicolor/scalable/apps/tubeglass.svg"
install -Dm644 LICENSE "$ROOT/data/usr/share/doc/tubeglass/copyright"

mkdir -p "$ROOT/control"
INSTALLED_KB=$(du -sk "$ROOT/data" | cut -f1)
cat > "$ROOT/control/control" <<CTL
Package: tubeglass
Version: $VERSION
Section: video
Priority: optional
Architecture: $ARCH
Depends: ffmpeg, libasound2 | libasound2t64, libgl1, libxkbcommon0
Recommends: xdg-desktop-portal
Installed-Size: $INSTALLED_KB
Maintainer: flawlesscode254 <flawlesscode254@users.noreply.github.com>
Homepage: https://github.com/flawlesscode254/TubeGlass
Description: YouTube, YouTube Music and podcast client
 Follow channels, watch live streams, listen to music and podcasts,
 download for offline use, and play local audio and video.
 No account or API key needed.
CTL
cat > "$ROOT/control/postinst" <<'POST'
#!/bin/sh
set -e
command -v update-desktop-database >/dev/null && update-desktop-database -q /usr/share/applications || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q /usr/share/icons/hicolor || true
exit 0
POST
chmod 755 "$ROOT/control/postinst"

mkdir -p dist
( cd "$ROOT/control" && tar --owner=0 --group=0 -czf "$ROOT/control.tar.gz" . )
( cd "$ROOT/data" && tar --owner=0 --group=0 -czf "$ROOT/data.tar.gz" . )
echo "2.0" > "$ROOT/debian-binary"
OUT="dist/tubeglass_${VERSION}_${ARCH}.deb"
rm -f "$OUT"
( cd "$ROOT" && ar rc "$OLDPWD/$OUT" debian-binary control.tar.gz data.tar.gz )
echo "Built $OUT"
