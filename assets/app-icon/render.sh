#!/usr/bin/env sh
# Renders every raster app icon from the SVG masters (assets/app-icon/README.md).
# Needs `resvg` (cargo install resvg); the .ico is packed by `cargo xtask ico`.
set -eu
cd "$(dirname "$0")/../.."
ICON=assets/app-icon
SVG=$ICON/scorecraft.svg
WEB=apps/scorecraft-web
ANDROID=apps/scorecraft-android/res
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
NOTE=$(sed -n 's/.*<path fill="#FFFFFF" d="\([^"]*\)".*/\1/p' "$SVG")

# Linux icon theme, and the window icon (256 px).
for s in 16 24 32 48 64 128 256 512; do
  mkdir -p "$ICON/hicolor/${s}x${s}/apps"
  resvg -w "$s" -h "$s" "$SVG" "$ICON/hicolor/${s}x${s}/apps/app.scorecraft.png"
done
mkdir -p "$ICON/hicolor/scalable/apps"
cp "$SVG" "$ICON/hicolor/scalable/apps/app.scorecraft.svg"

# macOS: the tile on Apple's 824/1024 grid.
cat > "$TMP/mac.svg" <<SVG
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024"><g transform="translate(100 100) scale(1.609375)"><rect width="512" height="512" rx="112" fill="#c45821"/><path fill="#FFFFFF" d="$NOTE"/></g></svg>
SVG
resvg -w 1024 -h 1024 "$TMP/mac.svg" "$ICON/scorecraft-1024.png"

# Full-bleed square (no rounded corners) for platforms that mask the icon themselves.
cat > "$TMP/square.svg" <<SVG
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512"><rect width="512" height="512" fill="#c45821"/><path fill="#FFFFFF" d="$NOTE"/></svg>
SVG

# Browser build: SVG favicon, PNG fallbacks, the manifest's icons and the iOS home-screen icon.
mkdir -p "$WEB/icons"
cp "$SVG" "$WEB/favicon.svg"
resvg -w 32 -h 32 "$SVG" "$WEB/icons/favicon-32.png"
resvg -w 192 -h 192 "$SVG" "$WEB/icons/icon-192.png"
resvg -w 512 -h 512 "$SVG" "$WEB/icons/icon-512.png"
resvg -w 512 -h 512 "$TMP/square.svg" "$WEB/icons/icon-maskable-512.png"
resvg -w 180 -h 180 "$TMP/square.svg" "$WEB/icons/apple-touch-icon.png"

# Android legacy launcher icons (API < 26); 26+ uses the adaptive icon in mipmap-anydpi-v26.
for d in mdpi:48 hdpi:72 xhdpi:96 xxhdpi:144 xxxhdpi:192; do
  mkdir -p "$ANDROID/mipmap-${d%%:*}"
  resvg -w "${d##*:}" -h "${d##*:}" "$SVG" "$ANDROID/mipmap-${d%%:*}/ic_launcher.png"
done

cargo xtask ico
