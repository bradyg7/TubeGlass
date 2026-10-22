#!/usr/bin/env bash
# Installs TubeGlass for the current user so it shows up in "Open with" for media files.
#   ./install.sh            build + install
#   ./install.sh --default  ...and make TubeGlass the default app for video/audio files
#   ./install.sh --uninstall
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

cargo build --release
mkdir -p "$BIN" "$APPS" "$ICONS"
install -m 755 target/release/tubeglass "$BIN/tubeglass"
install -m 644 packaging/tubeglass.svg "$ICONS/tubeglass.svg"
# point Exec at the installed binary so launchers work without ~/.local/bin on PATH
sed "s|^Exec=tubeglass|Exec=$BIN/tubeglass|" packaging/tubeglass.desktop > "$APPS/tubeglass.desktop"
chmod 644 "$APPS/tubeglass.desktop"
update-desktop-database "$APPS" 2>/dev/null || true
gtk-update-icon-cache -q "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

if [[ "${1:-}" == "--default" ]]; then
  mimes=$(grep '^MimeType=' packaging/tubeglass.desktop | cut -d= -f2 | tr ';' ' ')
  for m in $mimes; do xdg-mime default tubeglass.desktop "$m"; done
  echo "TubeGlass is now the default app for video and audio files."
fi

echo "Installed to $BIN/tubeglass — right-click a media file → Open With → TubeGlass."
