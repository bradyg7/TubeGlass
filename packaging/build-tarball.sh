#!/usr/bin/env bash
# Packs the built binary, ffmpeg, desktop file and icon into
# dist/TubeGlass-<version>-linux-<arch>.tar.gz with a small user-level installer.
# Set SKIP_BUILD=1 to package an already built target/release/tubeglass, SKIP_FFMPEG=1 to leave ffmpeg out.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
ARCH=$(uname -m)
[[ -n "${SKIP_BUILD:-}" ]] || cargo build --release --locked
NAME="TubeGlass-${VERSION}-linux-${ARCH}"
rm -rf "dist/$NAME"
mkdir -p "dist/$NAME"
install -m755 target/release/tubeglass "dist/$NAME/tubeglass"
install -m644 packaging/tubeglass.desktop "dist/$NAME/tubeglass.desktop"
install -m644 packaging/tubeglass.svg "dist/$NAME/tubeglass.svg"
install -m755 packaging/tarball-install.sh "dist/$NAME/install.sh"
if [[ -z "${SKIP_FFMPEG:-}" ]]; then
  case "$ARCH" in x86_64) FF=linux-x64 ;; aarch64) FF=linux-arm64 ;; *) FF= ;; esac
  [[ -z "$FF" ]] || ./packaging/fetch-ffmpeg.sh "$FF" "dist/$NAME"
fi
install -m644 README.md LICENSE "dist/$NAME/"
tar -C dist -czf "dist/$NAME.tar.gz" "$NAME"
rm -rf "dist/$NAME"
echo "Built dist/$NAME.tar.gz"
