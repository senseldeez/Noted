//! Notes on disk. Mirrors the Android app's Store.kt: notes are plain `.md` files, folders are
//! real folders, and each folder's hidden `.noted.json` keeps pins and manual order, so one
//! synced folder looks the same on phone and computer.
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const META: &str = ".noted.json";

pub const WELCOME: &str = "# Welcome to Noted
Your notes are plain **.md** files in this folder. Point a sync app at it to keep it in step with your phone.

## Formatting
Type a slash at the start of a line or after a space, then say what you want: *bullet point*, *todo*, *big header*, *line*.
- **Tab** or arrow keys move through the list
- **Enter** or a click inserts it
- **Esc** keeps a plain slash
";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Note,
    Folder,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub kind: Kind,
    pub path: PathBuf,
    /// File name on disk, e.g. `Groceries.md`.
    pub file: String,
    /// Display name, e.g. `Groceries`.
    pub name: String,
    pub text: String,
    pub mtime: f64,
    pub pinned: bool,
    pub children: Vec<Item>,
}

impl Item {
    pub fn is_folder(&self) -> bool {
        self.kind == Kind::Folder
    }
    /// Number of notes in this folder and every folder below it.
    pub fn note_count(&self) -> usize {
        self.children
            .iter()
            .map(|c| if c.is_folder() { c.note_count() } else { 1 })
            .sum()
    }
    pub fn find(&self, p: &Path) -> Option<&Item> {
        if self.path == p {
            return Some(self);
        }
        if !p.starts_with(&self.path) {
            return None;
        }
        self.children.iter().find_map(|c| c.find(p))
    }
    /// Every note below this folder, depth first.
    pub fn all_notes<'a>(&'a self, out: &mut Vec<&'a Item>) {
        for c in &self.children {
            if c.is_folder() {
                c.all_notes(out)
            } else {
                out.push(c)
            }
        }
    }
    /// Every folder (including this one) with its depth, for the Move dialog.
    pub fn all_folders<'a>(&'a self, depth: usize, out: &mut Vec<(&'a Item, usize)>) {
        out.push((self, depth));
        for c in &self.children {
            if c.is_folder() {
                c.all_folders(depth + 1, out)
            }
        }
    }
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Meta {
    #[serde(default)]
    pub pinned: Vec<String>,
    #[serde(default)]
    pub order: Vec<String>,
}

pub fn is_note(name: &str) -> bool {
    let l = name.to_lowercase();
    !name.starts_with('.') && (l.ends_with(".md") || l.ends_with(".markdown"))
}

pub fn strip_ext(name: &str) -> String {
    let l = name.to_lowercase();
    if l.ends_with(".markdown") {
        name[..name.len() - 9].to_string()
    } else if l.ends_with(".md") {
        name[..name.len() - 3].to_string()
    } else {
        name.to_string()
    }
}

pub fn sanitize(name: &str) -> String {
    let s: String = name
        .trim()
        .chars()
        .map(|c| if "\\/:*?\"<>|\n\r\t".contains(c) { '-' } else { c })
        .collect();
    let s = s.trim_start_matches('.').to_string();
    if s.is_empty() {
        "Untitled".into()
    } else {
        s
    }
}

fn mtime(p: &Path) -> f64 {
    fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn read_meta(dir: &Path) -> Meta {
    fs::read_to_string(dir.join(META))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn write_meta(dir: &Path, m: &Meta) {
    fn dedup(v: &[String]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for x in v {
            if !out.contains(x) {
                out.push(x.clone())
            }
        }
        out
    }
    let m = Meta { pinned: dedup(&m.pinned), order: dedup(&m.order) };
    if let Ok(s) = serde_json::to_string_pretty(&m) {
        let _ = fs::write(dir.join(META), s);
    }
}

fn replace_in_meta(dir: &Path, old: &str, new: Option<&str>) {
    let mut m = read_meta(dir);
    let mut changed = false;
    for v in [&mut m.pinned, &mut m.order] {
        if let Some(i) = v.iter().position(|x| x == old) {
            changed = true;
            match new {
                Some(n) => v[i] = n.to_string(),
                None => {
                    v.remove(i);
                }
            }
        }
    }
    if changed {
        write_meta(dir, &m)
    }
}

/// Folder tree, ordered like Store.list: pinned (in pin order), new items by date, then manual order.
pub fn scan(dir: &Path) -> Item {
    let meta = read_meta(dir);
    let mut kids = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let p = e.path();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                let mut sub = scan(&p);
                sub.mtime = mtime(&p);
                sub.pinned = meta.pinned.contains(&name);
                kids.push(sub);
            } else if ft.is_file() && is_note(&name) {
                let text = fs::read_to_string(&p).unwrap_or_default();
                kids.push(Item {
                    kind: Kind::Note,
                    name: strip_ext(&name),
                    file: name.clone(),
                    path: p.clone(),
                    text,
                    mtime: mtime(&p),
                    pinned: meta.pinned.contains(&name),
                    children: vec![],
                });
            }
        }
    }
    let mut pinned: Vec<Item> = meta
        .pinned
        .iter()
        .filter_map(|n| kids.iter().find(|k| &k.file == n).cloned())
        .collect();
    for p in &mut pinned {
        p.pinned = true
    }
    let rest: Vec<&Item> = kids.iter().filter(|k| !k.pinned).collect();
    let ordered: Vec<Item> = meta
        .order
        .iter()
        .filter_map(|n| rest.iter().find(|k| &k.file == n).map(|k| (*k).clone()))
        .collect();
    let mut unordered: Vec<Item> =
        rest.iter().filter(|k| !meta.order.contains(&k.file)).map(|k| (*k).clone()).collect();
    unordered.sort_by(|a, b| b.mtime.partial_cmp(&a.mtime).unwrap_or(std::cmp::Ordering::Equal));
    let file = dir.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut children = pinned;
    children.extend(unordered);
    children.extend(ordered);
    Item {
        kind: Kind::Folder,
        path: dir.to_path_buf(),
        name: file.clone(),
        file,
        text: String::new(),
        mtime: 0.0,
        pinned: false,
        children,
    }
}

fn unique(dir: &Path, base: &str, ext: &str) -> PathBuf {
    let mut f = dir.join(format!("{base}{ext}"));
    let mut i = 2;
    while f.exists() {
        f = dir.join(format!("{base} {i}{ext}"));
        i += 1;
    }
    f
}

pub fn create_note(dir: &Path, name: &str, text: &str) -> std::io::Result<PathBuf> {
    let f = unique(dir, &strip_ext(&sanitize(name)), ".md");
    fs::write(&f, text)?;
    Ok(f)
}

pub fn create_folder(dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    let f = unique(dir, &sanitize(name), "");
    fs::create_dir_all(&f)?;
    Ok(f)
}

pub fn write(p: &Path, text: &str) -> std::io::Result<()> {
    fs::write(p, text)
}

/// Rename a note or folder. Returns the new path, or None when the name is taken.
pub fn rename(p: &Path, new_name: &str) -> Option<PathBuf> {
    let dir = p.parent()?;
    let clean = sanitize(new_name);
    let target = if p.is_dir() { dir.join(&clean) } else { dir.join(format!("{}.md", strip_ext(&clean))) };
    if target == p {
        return Some(p.to_path_buf());
    }
    let same_ignoring_case =
        target.to_string_lossy().to_lowercase() == p.to_string_lossy().to_lowercase();
    if !same_ignoring_case && target.exists() {
        return None;
    }
    fs::rename(p, &target).ok()?;
    replace_in_meta(dir, &file_name(p), Some(&file_name(&target)));
    Some(target)
}

pub fn file_name(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for e in fs::read_dir(src)? {
        let e = e?;
        let t = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &t)?
        } else {
            fs::copy(e.path(), t)?;
        }
    }
    Ok(())
}

/// Move a note or folder into `dest`. Returns the new path.
pub fn move_to(src: &Path, dest: &Path) -> Option<PathBuf> {
    let from = src.parent()?;
    if from == dest {
        return Some(src.to_path_buf());
    }
    if src.is_dir() && dest.starts_with(src) {
        return None;
    }
    let name = file_name(src);
    let target = if src.is_dir() { unique(dest, &name, "") } else { unique(dest, &strip_ext(&name), ".md") };
    if fs::rename(src, &target).is_err() {
        if src.is_dir() {
            copy_dir(src, &target).ok()?;
            fs::remove_dir_all(src).ok()?;
        } else {
            fs::copy(src, &target).ok()?;
            fs::remove_file(src).ok()?;
        }
    }
    replace_in_meta(from, &name, None);
    Some(target)
}

/// Move to the system trash (recoverable).
pub fn trash(p: &Path) -> bool {
    let ok = trash::delete(p).is_ok();
    if ok {
        if let Some(dir) = p.parent() {
            replace_in_meta(dir, &file_name(p), None)
        }
    }
    ok
}

pub fn set_pinned(p: &Path, pinned: bool) {
    let Some(dir) = p.parent() else { return };
    let mut m = read_meta(dir);
    let n = file_name(p);
    m.pinned.retain(|x| x != &n);
    if pinned {
        m.pinned.push(n)
    }
    write_meta(dir, &m)
}

pub fn save_order(dir: &Path, pinned: Vec<String>, order: Vec<String>) {
    write_meta(dir, &Meta { pinned, order })
}

/* ---------- config ---------- */

#[derive(Serialize, Deserialize, Default)]
struct Config {
    root: Option<PathBuf>,
}

fn config_file() -> Option<PathBuf> {
    directories::ProjectDirs::from("app", "noted", "Noted").map(|d| d.config_dir().join("config.json"))
}

fn read_cfg() -> Config {
    config_file()
        .and_then(|f| fs::read_to_string(f).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_root(root: &Path) {
    if let Some(f) = config_file() {
        let _ = fs::create_dir_all(f.parent().unwrap());
        let c = Config { root: Some(root.to_path_buf()) };
        let _ = fs::write(f, serde_json::to_string_pretty(&c).unwrap_or_default());
    }
}

/// The notes folder: the saved one if it still exists, otherwise `Documents/Noted`.
pub fn ensure_root() -> PathBuf {
    if let Some(r) = read_cfg().root {
        if r.is_dir() {
            return r;
        }
    }
    let docs = directories::UserDirs::new()
        .and_then(|u| u.document_dir().map(|d| d.to_path_buf()))
        .or_else(|| {
            directories::BaseDirs::new().map(|b| {
                let docs = b.home_dir().join("Documents");
                if docs.is_dir() { docs } else { b.home_dir().to_path_buf() }
            })
        })
        .unwrap_or_else(|| PathBuf::from("."));
    let root = docs.join("Noted");
    let _ = fs::create_dir_all(&root);
    let empty = fs::read_dir(&root)
        .map(|rd| rd.flatten().all(|e| e.file_name().to_string_lossy().starts_with('.')))
        .unwrap_or(true);
    if empty {
        let _ = fs::write(root.join("Welcome.md"), WELCOME);
    }
    save_root(&root);
    root
}
