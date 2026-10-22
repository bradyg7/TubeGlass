#!/usr/bin/env bash
# Downloads the pinned ffmpeg for a platform, checks its SHA-256 and unpacks it.
#   ./packaging/fetch-ffmpeg.sh <platform> <destination-folder>
# <platform> is linux-x64, linux-arm64, darwin-x64, darwin-arm64 or win32-x64; the list of
# builds (and their checksums) is packaging/ffmpeg-sources.tsv, shared with the app itself.
set -euo pipefail

plat="${1:?platform missing (linux-x64, darwin-arm64, win32-x64 ...)}"
dest="${2:?destination folder missing}"
# make the destination absolute before changing into this script's folder
mkdir -p "$dest"
dest="$(cd "$dest" && pwd)"
cd "$(dirname "$0")"
line=$(awk -F'\t' -v p="$plat" '$1 == p { print $2 "\t" $3 }' ffmpeg-sources.tsv)
[[ -n "$line" ]] || { echo "no ffmpeg build listed for $plat" >&2; exit 1; }
url="${line%%$'\t'*}"
want="${line##*$'\t'}"

name=ffmpeg
[[ "$plat" == win32-* ]] && name=ffmpeg.exe
mkdir -p "$dest"
tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT

echo "Downloading ffmpeg for $plat ..."
curl -fsSL --retry 3 -o "$tmp" "$url"
if command -v sha256sum >/dev/null 2>&1; then got=$(sha256sum "$tmp" | cut -d' ' -f1); else got=$(shasum -a 256 "$tmp" | cut -d' ' -f1); fi
if [[ "$got" != "$want" ]]; then
  echo "checksum mismatch for $plat: expected $want, got $got" >&2
  exit 1
fi
gunzip -c "$tmp" > "$dest/$name"
chmod 755 "$dest/$name"
cp ffmpeg-LICENSE-GPLv3.txt THIRD_PARTY_NOTICES.md "$dest/" 2>/dev/null || true
echo "ffmpeg ready in $dest/$name"
