# Noted for desktop

The same notes, screens and gestures as the Android app, for Windows and macOS (Electron).
Notes are plain `.md` files in a folder you pick (default: `Documents/Noted`). Pins and manual
order are stored in each folder's hidden `.noted.json`, the same format the Android app uses, so
one synced folder looks the same on phone and computer.

Desktop additions: breadcrumbs, right-click menus, Ctrl/Cmd+click multi-select, keyboard
shortcuts (press `?`), drag-and-drop `.md` import, double-click to edit, and a slash menu for
formatting (`/` at the start of a line or after a space, or Ctrl/Cmd+/). Deleting moves files to
the system trash.

## Build

```sh
npm install
npm start           # run locally
npm run dist:win    # dist/Noted-<version>-portable.exe
npm run dist:mac    # dist/Noted-<version>-mac-arm64.zip and -mac-x64.zip
```

Both targets build on Linux. The Windows exe is branded by `build/afterPack.js` (no Wine needed).
The macOS app is not signed or notarized. After unzipping, move it to Applications and run once:

```sh
xattr -cr /Applications/Noted.app && codesign --force --deep --sign - /Applications/Noted.app
```
