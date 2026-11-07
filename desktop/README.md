# Noted for desktop

Native Rust (egui) version of Noted for Windows, macOS and Linux. Same notes, screens and
gestures as the Android app: notes are plain `.md` files in a folder you pick (default
`Documents/Noted`), and pins and manual order live in each folder's hidden `.noted.json`,
the format the Android app uses.

Desktop extras: breadcrumbs, right-click menus, Ctrl/Cmd+click multi-select, drag to reorder,
keyboard shortcuts (press `?`), drag-and-drop `.md` import, and a slash menu for formatting
(`/` at the start of a line or after a space, or Ctrl/Cmd+/). Deleting moves files to the
system trash.

## Build

```sh
cargo run                       # run locally
cargo test                      # markdown and editing tests
packaging/build-linux.sh 0.3.0  # AppImage, .deb, .tar.gz (needs cargo-deb, imagemagick)
packaging/build-macos.sh 0.3.0  # universal .app as .dmg and .zip (on macOS)
```

On Windows, `cargo build --release` gives a single portable `noted.exe` with its icon.

## Releases

`.github/workflows/build.yml` builds every platform plus the Android APK on each push and PR.
Pushing a `v*` tag, or running the workflow by hand with "release" ticked, publishes a GitHub
Release with all files attached. The APK is signed from the `ANDROID_KEYSTORE_BASE64` and
`ANDROID_KEYSTORE_PASSWORD` secrets.
