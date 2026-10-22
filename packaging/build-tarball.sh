#!/usr/bin/env bash
# Packs the built binary, desktop file and icon into
# dist/TubeGlass-<version>-linux-<arch>.tar.gz with a small user-level installer.
# Set SKIP_BUILD=1 to package an already built target/release/tubeglass.
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
install -m644 README.md LICENSE "dist/$NAME/"
tar -C dist -czf "dist/$NAME.tar.gz" "$NAME"
rm -rf "dist/$NAME"
echo "Built dist/$NAME.tar.gz"
