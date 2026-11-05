// Noted desktop: Electron shell around the same UI as the Android app.
// Notes are plain .md files in a folder you choose; pins and manual order live in a
// hidden .noted.json per folder, the same format the Android app writes, so a synced
// folder looks identical on both.
const { app, BrowserWindow, ipcMain, dialog, shell, Menu } = require('electron');
const fs = require('fs');
const fsp = fs.promises;
const path = require('path');

const META = '.noted.json';
const MAC = process.platform === 'darwin';
let win = null;

/* ---------- config ---------- */
const cfgFile = () => path.join(app.getPath('userData'), 'config.json');
function readCfg() { try { return JSON.parse(fs.readFileSync(cfgFile(), 'utf8')); } catch { return {}; } }
function writeCfg(c) { fs.mkdirSync(path.dirname(cfgFile()), { recursive: true }); fs.writeFileSync(cfgFile(), JSON.stringify(c, null, 2)); }

const WELCOME = `# Welcome to Noted
Your notes are plain **.md** files in this folder. Point a sync app at it to keep it in step with your phone.

## Formatting
Type a slash at the start of a line or after a space, then say what you want: *bullet point*, *todo*, *big header*, *line*.
- **Tab** or arrow keys move through the list
- **Enter** or a click inserts it
- **Esc** keeps a plain slash
`;

function ensureRoot() {
  const c = readCfg();
  if (c.root && fs.existsSync(c.root)) return c.root;
  const root = path.join(app.getPath('documents'), 'Noted');
  fs.mkdirSync(root, { recursive: true });
  if (!fs.readdirSync(root).some(n => !n.startsWith('.'))) fs.writeFileSync(path.join(root, 'Welcome.md'), WELCOME);
  writeCfg({ ...c, root });
  return root;
}
let ROOT = null;

/* ---------- file helpers (mirror Store.kt) ---------- */
const isNote = n => !n.startsWith('.') && /\.(md|markdown)$/i.test(n);
const stripExt = n => n.replace(/\.(md|markdown)$/i, '');
const sanitize = n => (n.trim().replace(/[\\/:*?"<>|\n\r\t]/g, '-').replace(/^\.+/, '') || 'Untitled');
function inside(p) {
  const r = path.resolve(ROOT), q = path.resolve(p);
  if (q !== r && !q.startsWith(r + path.sep)) throw new Error('Outside the notes folder');
  return q;
}
async function exists(p) { try { await fsp.access(p); return true; } catch { return false; } }
async function uniqueFile(dir, base, ext) {
  let f = path.join(dir, base + ext), i = 2;
  while (await exists(f)) f = path.join(dir, `${base} ${i++}${ext}`);
  return f;
}
async function readMeta(dir) {
  try { const o = JSON.parse(await fsp.readFile(path.join(dir, META), 'utf8')); return { pinned: o.pinned || [], order: o.order || [] }; }
  catch { return { pinned: [], order: [] }; }
}
async function writeMeta(dir, m) {
  const o = { pinned: [...new Set(m.pinned)], order: [...new Set(m.order)] };
  try { await fsp.writeFile(path.join(dir, META), JSON.stringify(o, null, 2)); } catch {}
}
async function replaceInMeta(dir, oldName, newName) {
  const m = await readMeta(dir);
  const p = m.pinned.indexOf(oldName), o = m.order.indexOf(oldName);
  if (p < 0 && o < 0) return;
  if (p >= 0) newName == null ? m.pinned.splice(p, 1) : (m.pinned[p] = newName);
  if (o >= 0) newName == null ? m.order.splice(o, 1) : (m.order[o] = newName);
  await writeMeta(dir, m);
}

/** Folder tree, ordered like Store.list: pinned (in pin order), new items by date, then manual order. */
async function scan(dir) {
  const meta = await readMeta(dir);
  let ents = [];
  try { ents = await fsp.readdir(dir, { withFileTypes: true }); } catch {}
  const kids = [];
  for (const e of ents) {
    if (e.name.startsWith('.')) continue;
    const p = path.join(dir, e.name);
    if (e.isDirectory()) {
      const st = await fsp.stat(p).catch(() => null); if (!st) continue;
      const sub = await scan(p);
      kids.push({ ...sub, mtime: st.mtimeMs, pinned: meta.pinned.includes(e.name) });
    } else if (e.isFile() && isNote(e.name)) {
      const st = await fsp.stat(p).catch(() => null); if (!st) continue;
      const text = await fsp.readFile(p, 'utf8').catch(() => '');
      kids.push({ type: 'note', path: p, file: e.name, name: stripExt(e.name), text, mtime: st.mtimeMs, pinned: meta.pinned.includes(e.name) });
    }
  }
  const by = new Map(kids.map(k => [k.file, k]));
  const pinned = meta.pinned.map(n => by.get(n)).filter(Boolean);
  const rest = kids.filter(k => !k.pinned);
  const ordered = meta.order.map(n => by.get(n)).filter(k => k && !k.pinned);
  const unordered = rest.filter(k => !meta.order.includes(k.file)).sort((a, b) => b.mtime - a.mtime);
  return { type: 'folder', path: dir, file: path.basename(dir), name: path.basename(dir), children: [...pinned, ...unordered, ...ordered] };
}

async function moveOne(src, destDir) {
  src = inside(src); destDir = inside(destDir);
  const from = path.dirname(src), name = path.basename(src);
  if (from === destDir) return src;
  const st = await fsp.stat(src);
  if (st.isDirectory() && (destDir + path.sep).startsWith(src + path.sep)) return null;
  const target = st.isDirectory() ? await uniqueFile(destDir, name, '') : await uniqueFile(destDir, stripExt(name), '.md');
  try { await fsp.rename(src, target); }
  catch { await fsp.cp(src, target, { recursive: true }); await fsp.rm(src, { recursive: true, force: true }); }
  await replaceInMeta(from, name, null);
  return target;
}

/* ---------- IPC ---------- */
const h = (ch, fn) => ipcMain.handle(ch, (_e, ...a) => fn(...a));
h('root', () => ROOT);
h('scan', () => scan(ROOT));
h('chooseRoot', async () => {
  const r = await dialog.showOpenDialog(win, { title: 'Choose your notes folder', defaultPath: ROOT, properties: ['openDirectory', 'createDirectory'] });
  if (r.canceled || !r.filePaths[0]) return null;
  ROOT = r.filePaths[0]; writeCfg({ ...readCfg(), root: ROOT });
  return ROOT;
});
h('write', async (p, text) => { await fsp.writeFile(inside(p), text); return (await fsp.stat(p)).mtimeMs; });
ipcMain.on('writeSync', (e, p, text) => { try { fs.writeFileSync(inside(p), text); e.returnValue = true; } catch { e.returnValue = false; } });
h('createNote', async (dir, name) => { const f = await uniqueFile(inside(dir), stripExt(sanitize(name)), '.md'); await fsp.writeFile(f, ''); return f; });
h('createFolder', async (dir, name) => { const f = await uniqueFile(inside(dir), sanitize(name), ''); await fsp.mkdir(f, { recursive: true }); return f; });
h('rename', async (p, newName) => {
  p = inside(p);
  const dir = path.dirname(p), st = await fsp.stat(p), clean = sanitize(newName);
  const target = st.isDirectory() ? path.join(dir, clean) : path.join(dir, stripExt(clean) + '.md');
  if (target === p) return p;
  if (target.toLowerCase() !== p.toLowerCase() && await exists(target)) return null;
  await fsp.rename(p, target);
  await replaceInMeta(dir, path.basename(p), path.basename(target));
  return target;
});
h('move', async (paths, dest) => { const out = {}; for (const p of paths) out[p] = await moveOne(p, dest).catch(() => null); return out; });
h('trash', async paths => {
  let n = 0;
  for (const p of paths) { try { await shell.trashItem(inside(p)); await replaceInMeta(path.dirname(p), path.basename(p), null); n++; } catch {} }
  return n;
});
h('setPinned', async (paths, pinned) => {
  for (const p of paths) {
    const dir = path.dirname(inside(p)), m = await readMeta(dir), n = path.basename(p);
    m.pinned = m.pinned.filter(x => x !== n); if (pinned) m.pinned.push(n);
    await writeMeta(dir, m);
  }
});
h('saveOrder', async (dir, pinnedNames, orderNames) => { await writeMeta(inside(dir), { pinned: pinnedNames, order: orderNames }); });
h('import', async (dir, files) => {
  let ok = 0, skipped = 0;
  for (const f of files) {
    if (!/\.(md|markdown)$/i.test(f.name)) { skipped++; continue; }
    try { const t = await uniqueFile(inside(dir), stripExt(sanitize(f.name)), '.md'); await fsp.writeFile(t, f.text); ok++; } catch { skipped++; }
  }
  return { ok, skipped };
});
h('reveal', p => shell.showItemInFolder(inside(p)));

/* ---------- window ---------- */
function createWindow() {
  win = new BrowserWindow({
    width: 1240, height: 840, minWidth: 560, minHeight: 480,
    backgroundColor: '#000000', title: 'Noted', show: false,
    titleBarStyle: 'hidden',
    ...(MAC ? { trafficLightPosition: { x: 16, y: 13 } } : { titleBarOverlay: { color: '#000000', symbolColor: '#8f8f8f', height: 38 } }),
    webPreferences: { preload: path.join(__dirname, 'preload.js'), contextIsolation: true, nodeIntegration: false, sandbox: true, spellcheck: false },
  });
  win.loadFile(path.join(__dirname, 'renderer', 'index.html'));
  win.once('ready-to-show', () => win.show());
  win.webContents.setWindowOpenHandler(({ url }) => { if (/^https?:|^mailto:/.test(url)) shell.openExternal(url); return { action: 'deny' }; });
  win.webContents.on('will-navigate', (e, url) => { if (!url.startsWith('file:')) { e.preventDefault(); shell.openExternal(url); } });
  win.on('focus', () => win.webContents.send('focus'));
}

function buildMenu() {
  if (!MAC) { Menu.setApplicationMenu(null); return; }
  // Undo/redo are handled by the editor itself, so the Edit menu leaves them out.
  Menu.setApplicationMenu(Menu.buildFromTemplate([
    { role: 'appMenu' },
    { label: 'Edit', submenu: [{ role: 'cut' }, { role: 'copy' }, { role: 'paste' }, { role: 'selectAll' }] },
    { role: 'windowMenu' },
  ]));
}

app.whenReady().then(() => {
  ROOT = ensureRoot();
  buildMenu();
  createWindow();
  app.on('activate', () => { if (BrowserWindow.getAllWindows().length === 0) createWindow(); });
});
app.on('window-all-closed', () => { if (!MAC) app.quit(); });
