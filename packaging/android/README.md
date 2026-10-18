# VectorCraft for Android

An Android app that runs the browser build of VectorCraft (`apps/vectorcraft-web`) offline in a
WebView. Narrow screens get the phone layout (`crates/ui-egui/src/mobile.rs`): a top bar with Menu,
Home, Undo, Redo and Panels; the tools in a bar along the bottom; panels in a bottom sheet.

| Piece | What it does |
|---|---|
| `build-web.sh` | `trunk build --release`, then copies `dist/web` into `app/src/main/assets/web` and the app icon into `res/mipmap-*` |
| `MainActivity.kt` | WebView on `https://appassets.androidplatform.net/` (WebViewAssetLoader), system file picker for Open/Place, system "Save as" for Save/Export, links open in the browser, Back = Escape (twice quickly: exit) |
| `.github/workflows/android.yml` | Builds the APK on GitHub's machines: no Android SDK needed on your computer |

## Build on GitHub (easiest)

1. Push this repository (or your fork) to GitHub.
2. Actions → **Android APK** → Run workflow.
3. When it finishes, download the `vectorcraft-android` artifact: it holds `app-debug.apk`.
4. Copy it to the phone and install it (allow "Install unknown apps" for your file manager).

A signed release APK (needed for Google Play or for updates without uninstalling): add the
repository secrets `VC_KEYSTORE_BASE64` (your `.jks`, base64), `VC_KEYSTORE_PASSWORD`,
`VC_KEY_ALIAS` and `VC_KEY_PASSWORD`; the workflow then also builds `app-release.apk`.

## Build on your computer

Needs Rust with `rustup target add wasm32-unknown-unknown`, `cargo install trunk --locked`,
JDK 17 and the Android SDK (Android Studio installs both).

```sh
packaging/android/build-web.sh          # trunk build + copy into the Android project
cd packaging/android
gradle assembleDebug                    # or open this folder in Android Studio and press Run
# APK: app/build/outputs/apk/debug/app-debug.apk
```

No Gradle wrapper is committed; Android Studio or a system `gradle` (8.9 or later) works.

## Limits

- Graphics: Android WebView usually has no WebGPU yet, so the app uses its WebGL2 fallback.
- Single-threaded rendering (the web build), so very large documents are slower than desktop.
- Fonts: only the fonts bundled with the web build; the phone's own fonts aren't listed.
- Dialogs (New Document, Export…) are the desktop ones; they fit a phone but are dense.
