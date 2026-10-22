#!/usr/bin/env bash
# Installs the TubeGlass in this folder for the current user (no root needed).
#   ./install.sh              install
#   ./install.sh --uninstall  remove
set -euo pipefail
cd "$(dirname "$0")"

BIN="$HOME/.local/bin"
APPS="$HOME/.local/share/applications"
ICONS="$HOME/.local/share/icons/hicolor/scalable/apps"

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -rf "$BIN/tubeglass" "$APPS/tubeglass.desktop" "$ICONS/tubeglass.svg" "$HOME/.local/lib/tubeglass"
  update-desktop-database "$APPS" 2>/dev/null || true
  echo "TubeGlass removed."
  exit 0
fi

LIB="$HOME/.local/lib/tubeglass"
mkdir -p "$BIN" "$APPS" "$ICONS" "$LIB"
install -m755 tubeglass "$BIN/tubeglass"
# the bundled ffmpeg goes where the program looks for it (../lib/tubeglass)
if [[ -f ffmpeg ]]; then
  install -m755 ffmpeg "$LIB/ffmpeg"
  install -m644 THIRD_PARTY_NOTICES.md ffmpeg-LICENSE-GPLv3.txt "$LIB/" 2>/dev/null || true
fi
install -m644 tubeglass.svg "$ICONS/tubeglass.svg"
sed "s|^Exec=tubeglass|Exec=$BIN/tubeglass|" tubeglass.desktop > "$APPS/tubeglass.desktop"
chmod 644 "$APPS/tubeglass.desktop"
update-desktop-database "$APPS" 2>/dev/null || true
gtk-update-icon-cache -q "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
echo "Installed to $BIN/tubeglass."
[[ -f "$LIB/ffmpeg" ]] || echo "No ffmpeg came with this package: TubeGlass will download one the first time it starts."
