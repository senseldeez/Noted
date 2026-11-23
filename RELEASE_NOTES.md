Noted is a minimal, pure-black markdown editor. Notes are plain `.md` files in a folder you choose, so any sync app can keep phone and computer in step.

### Downloads
| Platform | File |
|---|---|
| Windows 10/11 | `Noted-*-windows-x64.exe` (portable, no install) |
| macOS 11+ (Apple silicon and Intel) | `Noted-*-macos-universal.dmg` or `.zip` |
| Linux | `.AppImage`, `.deb`, or `.tar.gz` |
| Android 11+ | `Noted-*-android.apk` |

### What's new
- **Move blocks around.** The line you're editing now has a ⋮ handle at the end of its pill (on desktop also on any line you hover). Drag it to move the line, a list item with its sub-items, a heading with its whole section, or a code block; everything else slides out of the way. Tap or click it for Cut, Copy, Duplicate, Move up, Move down and Delete. On desktop Alt+↑/↓ moves the block too.
- **Live sync.** An open note follows its file on disk, so when a sync app (Google Drive and the like) brings in an edit from another device, it shows up right away, in reading and editing mode. Anything you typed meanwhile is kept.
- **Copy and paste work** again on desktop, and on Android through a Cut / Copy / Paste / Select all bar above the toolbar whenever text is selected.
- **Wrapped lines keep their indentation**: a long list item, quote or indented line wraps under its text instead of back at the left edge.
- A cleaner active-line pill (near-black with a fine outline) and rounded, softer text selection.
- Double-tap (or double-click) anywhere beside the text in reading mode to start editing there, not just on the words.

### First launch on macOS
The app is not notarized by Apple. After moving it to Applications, right-click it and choose **Open**, or run:
```
xattr -cr /Applications/Noted.app
```
