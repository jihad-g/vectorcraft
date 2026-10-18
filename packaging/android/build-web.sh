#!/usr/bin/env bash
# Fill the Android project with the browser build and the app icon, ready for Gradle.
#
# Usage: packaging/android/build-web.sh [--skip-build]
#
# Needs: trunk and the wasm32-unknown-unknown target (rustup target add wasm32-unknown-unknown;
# cargo install trunk --locked). Without --skip-build it runs `trunk build --release` first.
# Then: cd packaging/android && gradle assembleDebug   (or open the folder in Android Studio).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"

if [ "${1:-}" != "--skip-build" ]; then
  command -v trunk >/dev/null || { echo "error: trunk not found (cargo install trunk --locked)" >&2; exit 1; }
  (cd "$ROOT/apps/vectorcraft-web" && trunk build --release)
fi

SITE="$ROOT/dist/web"
[ -f "$SITE/index.html" ] || { echo "error: $SITE/index.html missing; run without --skip-build" >&2; exit 1; }

ASSETS="$HERE/app/src/main/assets/web"
rm -rf "$ASSETS"
mkdir -p "$ASSETS"
cp -R "$SITE/." "$ASSETS/"

# Launcher icons from the app icon (MIT OR Apache-2.0, assets/app-icon/LICENSE.txt).
RES="$HERE/app/src/main/res"
ICONS="$ROOT/assets/app-icon/hicolor"
for pair in mdpi:48x48 hdpi:64x64 xhdpi:128x128 xxhdpi:256x256 xxxhdpi:256x256; do
  dpi="${pair%%:*}"; size="${pair#*:}"
  mkdir -p "$RES/mipmap-$dpi"
  cp "$ICONS/$size/apps/ai.storyteller.vectorcraft.png" "$RES/mipmap-$dpi/ic_launcher.png"
done

echo "web build copied to $ASSETS ($(du -sh "$ASSETS" | cut -f1))"
