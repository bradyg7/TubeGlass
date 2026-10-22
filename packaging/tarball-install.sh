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
  rm -f "$BIN/tubeglass" "$APPS/tubeglass.desktop" "$ICONS/tubeglass.svg"
  update-desktop-database "$APPS" 2>/dev/null || true
  echo "TubeGlass removed."
  exit 0
fi

mkdir -p "$BIN" "$APPS" "$ICONS"
install -m755 tubeglass "$BIN/tubeglass"
install -m644 tubeglass.svg "$ICONS/tubeglass.svg"
sed "s|^Exec=tubeglass|Exec=$BIN/tubeglass|" tubeglass.desktop > "$APPS/tubeglass.desktop"
chmod 644 "$APPS/tubeglass.desktop"
update-desktop-database "$APPS" 2>/dev/null || true
gtk-update-icon-cache -q "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
echo "Installed to $BIN/tubeglass. TubeGlass also needs ffmpeg (e.g. sudo apt install ffmpeg)."
