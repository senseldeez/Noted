#!/usr/bin/env bash
# Build a universal (Apple silicon + Intel) Noted.app, then zip and dmg it.
# Run on macOS from desktop/: packaging/build-macos.sh 0.3.0
set -euo pipefail
V="$1"
cargo build --release --locked --target aarch64-apple-darwin
cargo build --release --locked --target x86_64-apple-darwin
APP=dist/Noted.app
rm -rf dist && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/noted" \
  target/aarch64-apple-darwin/release/noted target/x86_64-apple-darwin/release/noted
sed "s/VERSION/$V/g" packaging/Info.plist > "$APP/Contents/Info.plist"
# Icon set from the 1024px PNG.
ICONSET=dist/Noted.iconset
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z $s $s assets/icon.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  sips -z $((s*2)) $((s*2)) assets/icon.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Noted.icns"
rm -rf "$ICONSET"
# Ad-hoc signature so Apple silicon runs it (not notarized).
codesign --force --deep --sign - "$APP"
(cd dist && ditto -c -k --keepParent Noted.app "Noted-$V-macos-universal.zip")
mkdir -p dist/dmg && cp -R "$APP" dist/dmg/ && ln -s /Applications dist/dmg/Applications
hdiutil create -volname "Noted" -srcfolder dist/dmg -ov -format UDZO "dist/Noted-$V-macos-universal.dmg"
rm -rf dist/dmg
