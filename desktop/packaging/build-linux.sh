#!/usr/bin/env bash
# Build the Linux tarball, .deb and AppImage. Run from desktop/: packaging/build-linux.sh 0.3.0
set -euo pipefail
V="$1"
cargo build --release --locked
mkdir -p dist
# Tarball
T="dist/noted-$V-linux-x86_64"
mkdir -p "$T"
cp target/release/noted packaging/noted.desktop assets/icon.png "$T/"
tar -C dist -czf "dist/Noted-$V-linux-x86_64.tar.gz" "noted-$V-linux-x86_64"
rm -rf "$T"
# Debian package
cargo deb --no-build --output "dist/Noted-$V-linux-amd64.deb"
# AppImage
A=dist/Noted.AppDir
mkdir -p "$A/usr/bin" "$A/usr/share/applications" "$A/usr/share/icons/hicolor/256x256/apps"
cp target/release/noted "$A/usr/bin/"
cp packaging/noted.desktop "$A/" && cp packaging/noted.desktop "$A/usr/share/applications/"
convert assets/icon.png -resize 256x256 "$A/noted.png"
cp "$A/noted.png" "$A/usr/share/icons/hicolor/256x256/apps/noted.png"
cat > "$A/AppRun" <<'RUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/noted" "$@"
RUN
chmod +x "$A/AppRun"
curl -sSL -o dist/appimagetool https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
chmod +x dist/appimagetool
ARCH=x86_64 dist/appimagetool --appimage-extract-and-run "$A" "dist/Noted-$V-linux-x86_64.AppImage"
rm -rf "$A" dist/appimagetool
