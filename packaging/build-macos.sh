#!/usr/bin/env bash
# Builds TubeGlass.app and dist/TubeGlass-<version>-macos-<arch>.dmg (run on macOS).
#   ./packaging/build-macos.sh                 build for this Mac
#   ./packaging/build-macos.sh x86_64          build for Intel Macs (cross-compiles on Apple silicon)
# Set SKIP_BUILD=1 to package an already built binary.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
ARCH="${1:-$(uname -m)}"
[[ "$ARCH" == "arm64" ]] && ARCH=aarch64
TARGET="${ARCH}-apple-darwin"
if [[ -z "${SKIP_BUILD:-}" ]]; then
  rustup target add "$TARGET" >/dev/null 2>&1 || true
  cargo build --release --locked --target "$TARGET"
fi

APP="dist/TubeGlass.app"
rm -rf "$APP" dist/dmgroot
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
install -m755 "target/$TARGET/release/tubeglass" "$APP/Contents/MacOS/tubeglass"
install -m644 packaging/macos/tubeglass.icns "$APP/Contents/Resources/tubeglass.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>TubeGlass</string>
  <key>CFBundleDisplayName</key><string>TubeGlass</string>
  <key>CFBundleIdentifier</key><string>io.github.flawlesscode254.tubeglass</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleExecutable</key><string>tubeglass</string>
  <key>CFBundleIconFile</key><string>tubeglass</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.video</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
# an ad-hoc signature lets the app start on Apple silicon (it is not notarised)
codesign --force --deep --sign - "$APP" 2>/dev/null || true

mkdir -p dist/dmgroot
cp -R "$APP" dist/dmgroot/
ln -s /Applications dist/dmgroot/Applications
OUT="dist/TubeGlass-${VERSION}-macos-${ARCH}.dmg"
rm -f "$OUT"
hdiutil create -volname "TubeGlass" -srcfolder dist/dmgroot -ov -format UDZO "$OUT"
rm -rf dist/dmgroot
echo "Built $OUT"
