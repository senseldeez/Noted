Noted is a minimal, pure-black markdown editor. Notes are plain `.md` files in a folder you choose, so any sync app can keep phone and computer in step.

### Downloads
| Platform | File |
|---|---|
| Windows 10/11 | `Noted-*-windows-x64.exe` (portable, no install) |
| macOS 11+ (Apple silicon and Intel) | `Noted-*-macos-universal.dmg` or `.zip` |
| Linux | `.AppImage`, `.deb`, or `.tar.gz` |
| Android 11+ | `Noted-*-android.apk` |

### What's new
- The desktop app is rebuilt in native Rust (egui). It starts instantly, is a fraction of the old download size, and needs no browser engine.
- New builds for Linux, and one universal Mac app for Apple silicon and Intel.
- Android builds are now signed with a new release key. If you installed an earlier APK, uninstall it once before installing this one. Your notes are files in your folder and are not touched.

### First launch on macOS
The app is not notarized by Apple. After moving it to Applications, right-click it and choose **Open**, or run:
```
xattr -cr /Applications/Noted.app
```
