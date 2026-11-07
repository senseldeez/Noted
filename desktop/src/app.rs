//! App state, navigation, the title bar and header, and the overlays (menus, dialogs, search, toasts).
use crate::editor::Editor;
use crate::icons::{self, Icon};
use crate::store::{self, Item};
use crate::theme::*;
use crate::widgets::{self, ibtn, ibtn_sized, kbd, pillbtn, MOD};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, Key, Modifiers, Rect, Sense, Stroke, StrokeKind, Ui};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub const TITLEBAR: f32 = 38.0;
const HEADER: f32 = 66.0;

#[derive(Clone, PartialEq, Debug)]
pub enum Screen {
    Folder(PathBuf),
    Note(PathBuf),
}

#[derive(Clone, Debug)]
pub enum Action {
    OpenFolder(PathBuf),
    OpenNote(PathBuf),
    /// Open the editor, with the cursor at the end of the given line.
    EditNote(PathBuf, Option<usize>),
    Read,
    Back,
    /// Leave the editor straight back to the folder.
    BackToFolder,
    Crumb(usize),
    NewNote(PathBuf),
    NewFolder(PathBuf),
    Import(PathBuf),
    Rename(PathBuf),
    Move(Vec<PathBuf>),
    MoveTo(Vec<PathBuf>, PathBuf),
    /// Ask before moving to the trash; `true` when deleting the open note.
    Delete(Vec<PathBuf>, bool),
    Trash(Vec<PathBuf>, bool),
    Pin(Vec<PathBuf>, bool),
    Reveal(PathBuf),
    Refresh,
    ChooseRoot,
    Shortcuts,
    Search,
    Reorder { dir: PathBuf, pinned: Vec<String>, order: Vec<String> },
    WriteNote(PathBuf, String),
    ClearSelection,
    Undo,
    Redo,
}

pub struct MenuItem {
    pub icon: Icon,
    pub label: String,
    pub key: String,
    pub action: Option<Action>,
}

pub fn mi(icon: Icon, label: &str, key: &str, action: Action) -> MenuItem {
    MenuItem { icon, label: label.into(), key: key.into(), action: Some(action) }
}
pub fn sep() -> MenuItem {
    MenuItem { icon: Icon::Close, label: String::new(), key: String::new(), action: None }
}

/// Where a menu opens: under (or over) a button, right-aligned to it, or at the pointer.
#[derive(Clone, Copy)]
pub enum MenuAt {
    Below(Rect),
    Above(Rect),
    Point(egui::Pos2),
}

pub struct Menu {
    pub at: MenuAt,
    pub items: Vec<MenuItem>,
    pub opened_at: f64,
    pub hl: Option<usize>,
}

pub enum Dialog {
    NewNote { dir: PathBuf, value: String },
    NewFolder { dir: PathBuf, value: String },
    Rename { path: PathBuf, value: String },
    Move { paths: Vec<PathBuf> },
    Confirm { title: String, body: String, ok: String, action: Action },
    Shortcuts,
}

pub struct Search {
    pub query: String,
    pub sel: usize,
    pub focus: bool,
    pub opened_at: f64,
}

pub struct Drag {
    pub path: PathBuf,
    pub grab: egui::Vec2,
    pub pinned: bool,
    /// Current order of the dragged card's group while dragging.
    pub order: Vec<PathBuf>,
    pub started: f64,
}

pub struct App {
    pub root: PathBuf,
    pub tree: Item,
    pub stack: Vec<Screen>,
    pub editor: Option<Editor>,
    pub selected: HashSet<PathBuf>,
    pub drag: Option<Drag>,
    pub card_rects: Vec<(PathBuf, Rect)>,
    pub menu: Option<Menu>,
    pub dialog: Option<Dialog>,
    dialog_at: f64,
    pub search: Option<Search>,
    pub toast: Option<(String, f64)>,
    /// Folded headings per note, by the heading's text; shared by reader and editor.
    pub folds: HashMap<PathBuf, HashSet<String>>,
    /// Folded list items per note, by line.
    pub list_folds: HashMap<PathBuf, HashSet<usize>>,
    pub actions: Vec<Action>,
    was_focused: bool,
    dialog_focus: bool,
    view_key: String,
    view_at: f64,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::style(&cc.egui_ctx);
        let root = store::ensure_root();
        let tree = store::scan(&root);
        App {
            stack: vec![Screen::Folder(root.clone())],
            root,
            tree,
            editor: None,
            selected: HashSet::new(),
            drag: None,
            card_rects: vec![],
            menu: None,
            dialog: None,
            dialog_at: 0.0,
            search: None,
            toast: None,
            folds: store::load_folds(),
            list_folds: HashMap::new(),
            actions: vec![],
            was_focused: true,
            dialog_focus: false,
            view_key: String::new(),
            view_at: -1.0,
        }
    }

    pub fn top(&self) -> &Screen {
        self.stack.last().unwrap()
    }

    pub fn editing(&self) -> bool {
        matches!((self.top(), &self.editor), (Screen::Note(p), Some(e)) if &e.path == p)
    }

    /// Folder the current screen belongs to.
    pub fn current_dir(&self) -> PathBuf {
        match self.top() {
            Screen::Folder(p) => p.clone(),
            Screen::Note(p) => p.parent().map(|x| x.to_path_buf()).unwrap_or(self.root.clone()),
        }
    }

    pub fn reload(&mut self) {
        self.tree = store::scan(&self.root);
        // Drop screens whose files disappeared.
        let tree = &self.tree;
        self.stack.retain(|s| match s {
            Screen::Folder(p) | Screen::Note(p) => tree.find(p).is_some(),
        });
        if self.stack.is_empty() {
            self.stack.push(Screen::Folder(self.root.clone()));
        }
        self.selected.retain(|p| tree.find(p).is_some());
    }

    pub fn toast(&mut self, msg: impl Into<String>, now: f64) {
        self.toast = Some((msg.into(), now));
    }

    fn save_editor(&mut self) {
        if let Some(e) = &mut self.editor {
            if let Err(msg) = e.save() {
                self.toast = Some((msg, 0.0));
            }
        }
    }

    fn close_editor(&mut self) {
        self.save_editor();
        self.editor = None;
        self.reload();
    }

    fn open_dialog(&mut self, d: Dialog, now: f64) {
        self.menu = None;
        self.dialog = Some(d);
        self.dialog_at = now;
        self.dialog_focus = true;
    }

    /// Replace a path everywhere after a rename or move.
    fn remap(&mut self, old: &Path, new: &Path) {
        let fix = |p: &mut PathBuf| {
            if let Ok(rest) = p.strip_prefix(old) {
                *p = new.join(rest);
            }
        };
        for s in &mut self.stack {
            match s {
                Screen::Folder(p) | Screen::Note(p) => fix(p),
            }
        }
        if let Some(e) = &mut self.editor {
            fix(&mut e.path)
        }
        let moved: Vec<PathBuf> = self.folds.keys().filter(|k| k.starts_with(old)).cloned().collect();
        if !moved.is_empty() {
            for mut k in moved {
                let v = self.folds.remove(&k).unwrap_or_default();
                fix(&mut k);
                self.folds.insert(k, v);
            }
            store::save_folds(&self.folds);
        }
    }

    /// Navigate to a note from anywhere (search results, new notes).
    fn open_note_anywhere(&mut self, p: &Path) {
        let mut stack = vec![Screen::Folder(self.root.clone())];
        let mut cur = self.root.clone();
        if let Some(parent) = p.parent() {
            if let Ok(rel) = parent.strip_prefix(&self.root) {
                for c in rel.components() {
                    cur = cur.join(c);
                    stack.push(Screen::Folder(cur.clone()));
                }
            }
        }
        stack.push(Screen::Note(p.to_path_buf()));
        self.stack = stack;
    }

    fn run(&mut self, ctx: &egui::Context, a: Action) {
        let now = ctx.input(|i| i.time);
        match a {
            Action::OpenFolder(p) => {
                self.selected.clear();
                self.stack.push(Screen::Folder(p));
            }
            Action::OpenNote(p) => {
                self.selected.clear();
                if self.editing() {
                    self.close_editor()
                }
                if matches!(self.top(), Screen::Note(_)) {
                    self.stack.pop();
                }
                self.stack.push(Screen::Note(p));
            }
            Action::EditNote(p, line) => {
                self.selected.clear();
                if !matches!(self.top(), Screen::Note(x) if *x == p) {
                    self.stack.push(Screen::Note(p.clone()));
                }
                if let Some(item) = self.tree.find(&p) {
                    self.editor = Some(Editor::new(item, line));
                }
            }
            Action::Read => self.close_editor(),
            Action::Back => {
                if !self.selected.is_empty() {
                    self.selected.clear();
                } else if self.editing() {
                    self.close_editor();
                } else if self.stack.len() > 1 {
                    self.stack.pop();
                }
            }
            Action::BackToFolder => {
                self.close_editor();
                if matches!(self.top(), Screen::Note(_)) && self.stack.len() > 1 {
                    self.stack.pop();
                }
            }
            Action::Crumb(i) => {
                if self.editing() {
                    self.close_editor()
                }
                self.selected.clear();
                self.stack.truncate(i + 1);
            }
            Action::NewNote(dir) => {
                self.save_editor();
                self.open_dialog(Dialog::NewNote { dir, value: String::new() }, now);
            }
            Action::NewFolder(dir) => {
                self.save_editor();
                self.open_dialog(Dialog::NewFolder { dir, value: String::new() }, now);
            }
            Action::Import(dir) => {
                if let Some(files) = rfd::FileDialog::new().set_title("Import notes").add_filter("Markdown", &["md", "markdown", "txt"]).pick_files() {
                    self.import(&dir, files, now);
                }
            }
            Action::Rename(p) => {
                let value = self.tree.find(&p).map(|i| i.name.clone()).unwrap_or_default();
                self.open_dialog(Dialog::Rename { path: p, value }, now);
            }
            Action::Move(paths) => self.open_dialog(Dialog::Move { paths }, now),
            Action::MoveTo(paths, dest) => {
                self.save_editor();
                let mut n = 0;
                for p in &paths {
                    if let Some(new) = store::move_to(p, &dest) {
                        self.remap(p, &new);
                        n += 1;
                    }
                }
                self.selected.clear();
                self.reload();
                let name = if dest == self.root { "Noted".to_string() } else { store::file_name(&dest) };
                self.toast(if n == paths.len() { format!("Moved to {name}") } else { format!("Moved {n} of {}", paths.len()) }, now);
            }
            Action::Delete(paths, from_note) => {
                let items: Vec<&Item> = paths.iter().filter_map(|p| self.tree.find(p)).collect();
                let folders = items.iter().filter(|i| i.is_folder()).count();
                let body = if items.len() == 1 {
                    if folders > 0 {
                        format!("“{}” and everything inside it will be moved to the trash.", items[0].name)
                    } else {
                        format!("“{}” will be moved to the trash.", items[0].name)
                    }
                } else {
                    format!("{} items will be moved to the trash{}", items.len(), if folders > 0 { ", including everything inside the folders." } else { "." })
                };
                self.open_dialog(Dialog::Confirm { title: "Delete?".into(), body, ok: "Delete".into(), action: Action::Trash(paths, from_note) }, now);
            }
            Action::Trash(paths, from_note) => {
                if let Some(e) = &self.editor {
                    if paths.iter().any(|p| e.path.starts_with(p)) {
                        self.editor = None;
                    }
                }
                let n = paths.iter().filter(|p| store::trash(p)).count();
                self.selected.clear();
                if from_note && matches!(self.top(), Screen::Note(_)) {
                    self.stack.pop();
                }
                self.reload();
                let total = paths.len();
                self.toast(
                    if n == total {
                        if n == 1 { "Moved to trash".to_string() } else { format!("Moved {n} items to trash") }
                    } else {
                        format!("Couldn't delete {} of {total}", total - n)
                    },
                    now,
                );
            }
            Action::Pin(paths, on) => {
                for p in &paths {
                    store::set_pinned(p, on)
                }
                self.selected.clear();
                self.reload();
                self.toast(if on { "Pinned" } else { "Unpinned" }, now);
            }
            Action::Reveal(p) => {
                let _ = opener::reveal(&p);
            }
            Action::Refresh => {
                self.save_editor();
                self.reload();
                self.toast("Refreshed", now);
            }
            Action::ChooseRoot => {
                if let Some(dir) = rfd::FileDialog::new().set_title("Choose your notes folder").set_directory(&self.root).pick_folder() {
                    self.close_editor();
                    store::save_root(&dir);
                    self.root = dir.clone();
                    self.stack = vec![Screen::Folder(dir)];
                    self.reload();
                    self.toast("Notes folder changed", now);
                }
            }
            Action::Shortcuts => self.open_dialog(Dialog::Shortcuts, now),
            Action::Search => {
                self.save_editor();
                self.menu = None;
                self.search = Some(Search { query: String::new(), sel: 0, focus: true, opened_at: now })
            }
            Action::Reorder { dir, pinned, order } => {
                store::save_order(&dir, pinned, order);
                self.reload();
            }
            Action::WriteNote(p, text) => {
                if store::write(&p, &text).is_ok() {
                    self.reload();
                }
            }
            Action::ClearSelection => self.selected.clear(),
            Action::Undo => {
                if let Some(e) = &mut self.editor {
                    e.undo(ctx)
                }
            }
            Action::Redo => {
                if let Some(e) = &mut self.editor {
                    e.redo(ctx)
                }
            }
        }
    }

    fn import(&mut self, dir: &Path, files: Vec<PathBuf>, now: f64) {
        let (mut ok, mut skipped) = (0, 0);
        for p in files {
            let name = store::file_name(&p);
            if p.as_os_str().is_empty() || !(store::is_note(&name) || name.to_lowercase().ends_with(".txt")) {
                skipped += 1;
                continue;
            }
            let base = name.rsplit_once('.').map(|x| x.0.to_string()).unwrap_or(name.clone());
            match std::fs::read_to_string(&p).ok().and_then(|t| store::create_note(dir, &base, &t).ok()) {
                Some(_) => ok += 1,
                None => skipped += 1,
            }
        }
        self.reload();
        let msg = match (ok, skipped) {
            (0, _) => "Only .md files can be imported".to_string(),
            (1, 0) => "Imported 1 note".to_string(),
            (n, 0) => format!("Imported {n} notes"),
            (n, s) => format!("Imported {n}, skipped {s}"),
        };
        self.toast(msg, now);
    }

    /* ---------- keys ---------- */

    fn global_keys(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();
        let cmd = Modifiers::COMMAND;
        let (f, k, sn, n, e) = ctx.input_mut(|i| {
            let sn = i.consume_key(cmd | Modifiers::SHIFT, Key::N);
            (i.consume_key(cmd, Key::F), i.consume_key(cmd, Key::K) && !typing, sn, i.consume_key(cmd, Key::N), i.consume_key(cmd, Key::E))
        });
        let dir = self.current_dir();
        if (f || k) && self.dialog.is_none() {
            self.actions.push(Action::Search)
        }
        if self.dialog.is_none() {
            if sn {
                self.actions.push(Action::NewFolder(dir.clone()))
            } else if n {
                self.actions.push(Action::NewNote(dir.clone()))
            }
        }
        if e && self.dialog.is_none() && self.search.is_none() {
            match self.top().clone() {
                Screen::Note(p) if !self.editing() => self.actions.push(Action::EditNote(p, None)),
                Screen::Note(_) => self.actions.push(Action::Read),
                _ => {}
            }
        }
        if self.dialog.is_some() || self.search.is_some() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.dialog = None;
                self.search = None;
            }
            return;
        }
        if self.menu.is_some() {
            self.menu_keys(ctx);
            return;
        }
        if self.editing() {
            if self.editor.as_ref().map(|e| e.palette.is_none()).unwrap_or(true) && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.actions.push(Action::Read);
            }
            return;
        }
        if typing {
            return;
        }
        let (esc, alt_left, e_key, enter, q, f2, del) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Escape),
                i.consume_key(Modifiers::ALT, Key::ArrowLeft),
                i.consume_key(Modifiers::NONE, Key::E),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.events.iter().any(|e| matches!(e, egui::Event::Text(t) if t == "?")),
                i.consume_key(Modifiers::NONE, Key::F2),
                i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace) && i.modifiers.command,
            )
        });
        if esc || alt_left {
            self.actions.push(Action::Back)
        }
        if let Screen::Note(p) = self.top().clone() {
            if e_key || enter {
                self.actions.push(Action::EditNote(p.clone(), None))
            }
            if f2 {
                self.actions.push(Action::Rename(p))
            }
        } else {
            let sel: Vec<PathBuf> = self.selected.iter().cloned().collect();
            if f2 && sel.len() == 1 {
                self.actions.push(Action::Rename(sel[0].clone()))
            }
            if del && !sel.is_empty() {
                self.actions.push(Action::Delete(sel.clone(), false))
            }
            if enter && sel.len() == 1 {
                let p = sel[0].clone();
                self.actions.push(if p.is_dir() { Action::OpenFolder(p) } else { Action::OpenNote(p) });
            }
        }
        if q {
            self.actions.push(Action::Shortcuts)
        }
    }

    fn menu_keys(&mut self, ctx: &egui::Context) {
        let Some(m) = &mut self.menu else { return };
        let (esc, up, down, enter) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Escape),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        let idx: Vec<usize> = m.items.iter().enumerate().filter(|(_, it)| it.action.is_some()).map(|(i, _)| i).collect();
        if idx.is_empty() {
            return;
        }
        let pos = m.hl.and_then(|h| idx.iter().position(|&x| x == h));
        if down {
            m.hl = Some(idx[pos.map(|p| (p + 1) % idx.len()).unwrap_or(0)]);
        }
        if up {
            m.hl = Some(idx[pos.map(|p| (p + idx.len() - 1) % idx.len()).unwrap_or(idx.len() - 1)]);
        }
        if enter {
            if let Some(a) = m.hl.and_then(|h| m.items[h].action.clone()) {
                self.actions.push(a);
                self.menu = None;
                return;
            }
        }
        if esc {
            self.menu = None;
        }
    }

    /* ---------- title bar ---------- */

    fn titlebar(&mut self, ui: &mut Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let bg = ui.interact(rect, ui.id().with("titlebar"), Sense::click_and_drag());
        if bg.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        if bg.double_clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }
        widgets::text(ui, rect.center(), Align2::CENTER_CENTER, "Noted", bold(12.0), MUTED);
        if cfg!(target_os = "macos") {
            return;
        }
        // Window buttons on Windows and Linux, where the app draws its own frame.
        let mut x = rect.max.x - 8.0;
        let cy = rect.center().y;
        let buttons = [
            (Icon::Close, "Close", egui::ViewportCommand::Close),
            (if maximized { Icon::Restore } else { Icon::Maximize }, if maximized { "Restore" } else { "Maximize" }, egui::ViewportCommand::Maximized(!maximized)),
            (Icon::Minimize, "Minimize", egui::ViewportCommand::Minimized(true)),
        ];
        for (icon, label, cmd) in buttons {
            let r = Rect::from_min_max(pos2(x - 40.0, cy - 14.0), pos2(x, cy + 14.0));
            let id = ui.id().with(("wc", label));
            let resp = ui.interact(r, id, Sense::click());
            let h = ctx.animate_bool_with_time(id, resp.hovered(), 0.12);
            if h > 0.0 {
                let fill = if icon == Icon::Close { Color32::from_rgb(0xc4, 0x2b, 0x1c) } else { PILL };
                ui.painter().rect_filled(r, 8, fill.gamma_multiply(h));
            }
            icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(14.0, 14.0)), icon, widgets::lerp_color(MUTED, WHITE, h));
            if resp.clicked() {
                ctx.send_viewport_cmd(cmd);
            }
            x -= 42.0;
        }
    }

    /// Resize handles along the window edges, since the native frame is hidden.
    fn resize_edges(&mut self, ctx: &egui::Context) {
        if cfg!(target_os = "macos") || ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
            return;
        }
        use egui::viewport::ResizeDirection as D;
        let screen = ctx.content_rect();
        let Some(p) = ctx.input(|i| i.pointer.hover_pos()) else { return };
        if ctx.dragged_id().is_some() {
            return;
        }
        let m = 6.0;
        let (l, r, t, b) = (p.x < screen.min.x + m, p.x > screen.max.x - m, p.y < screen.min.y + m, p.y > screen.max.y - m);
        let dir = match (l, r, t, b) {
            (true, _, true, _) => Some((D::NorthWest, egui::CursorIcon::ResizeNorthWest)),
            (_, true, true, _) => Some((D::NorthEast, egui::CursorIcon::ResizeNorthEast)),
            (true, _, _, true) => Some((D::SouthWest, egui::CursorIcon::ResizeSouthWest)),
            (_, true, _, true) => Some((D::SouthEast, egui::CursorIcon::ResizeSouthEast)),
            (true, ..) => Some((D::West, egui::CursorIcon::ResizeWest)),
            (_, true, ..) => Some((D::East, egui::CursorIcon::ResizeEast)),
            (_, _, true, _) => Some((D::North, egui::CursorIcon::ResizeNorth)),
            (_, _, _, true) => Some((D::South, egui::CursorIcon::ResizeSouth)),
            _ => None,
        };
        if let Some((d, icon)) = dir {
            ctx.set_cursor_icon(icon);
            if ctx.input(|i| i.pointer.primary_pressed()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(d));
            }
        }
    }

    /* ---------- header ---------- */

    fn header(&mut self, ui: &mut Ui, rect: Rect) {
        let top = self.top().clone();
        let is_root = self.stack.len() == 1;
        let cy = rect.min.y + 14.0 + 22.0;
        let mut x = rect.min.x + if is_root && matches!(top, Screen::Folder(_)) { 26.0 } else { 10.0 };
        let mut rx = rect.max.x - 18.0;
        let btn = |cx: f32| Rect::from_center_size(pos2(cx, cy), vec2(44.0, 44.0));

        // Selecting cards: just the count and Done.
        if matches!(top, Screen::Folder(_)) && !self.selected.is_empty() {
            let n = self.selected.len();
            let label = if n == 1 { "Drag to reorder".to_string() } else { format!("{n} selected") };
            widgets::text(ui, pos2(rect.min.x + 26.0 + 16.0, cy), Align2::LEFT_CENTER, &label, sans(13.0), TEXT3);
            let w = widgets::pill_width(ui, "Done", false);
            if pillbtn(ui, Rect::from_min_max(pos2(rx - w, cy - 20.0), pos2(rx, cy + 20.0)), "Done", None, true).clicked() {
                self.actions.push(Action::ClearSelection);
            }
            return;
        }

        let editing = self.editing();
        if !(is_root && matches!(top, Screen::Folder(_))) {
            let (tip, act) = if editing { ("Back to folder", Action::BackToFolder) } else { ("Back", Action::Back) };
            if ibtn(ui, btn(x + 22.0), Icon::Back, tip, "Esc", false).clicked() {
                self.actions.push(act);
            }
            x += 48.0;
        }

        match &top {
            Screen::Folder(dir) => {
                let more = btn(rx - 22.0);
                if ibtn(ui, more, Icon::More, "More", "", false).clicked() {
                    let items = vec![
                        mi(Icon::Refresh, "Refresh", "", Action::Refresh),
                        mi(Icon::Folder, "Change notes folder", "", Action::ChooseRoot),
                        mi(Icon::Import, "Show in file manager", "", Action::Reveal(dir.clone())),
                        sep(),
                        mi(Icon::Keys, "Keyboard shortcuts", "?", Action::Shortcuts),
                    ];
                    self.open_menu(ui, MenuAt::Below(more), items);
                }
                rx -= 48.0;
                // Search pill.
                let narrow = rect.width() < 720.0;
                let w = if narrow { 44.0 } else { 210.0_f32.max(0.0) };
                let r = Rect::from_min_max(pos2(rx - w, cy - 20.0), pos2(rx, cy + 20.0));
                let resp = ui.interact(r, ui.id().with("searchpill"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                let h = ui.ctx().animate_bool_with_time(ui.id().with("searchpill-h"), resp.hovered(), 0.12);
                ui.painter().rect_stroke(r, 20, Stroke::new(LINE, widgets::lerp_color(BORDER, Color32::from_gray(0x55), h)), StrokeKind::Inside);
                let ic = if narrow { r.center() } else { pos2(r.min.x + 12.0 + 8.5, cy) };
                icons::paint(ui.painter(), Rect::from_center_size(ic, vec2(17.0, 17.0)), Icon::Search, widgets::lerp_color(MUTED, TEXT2, h));
                if !narrow {
                    widgets::text(ui, pos2(r.min.x + 39.0, cy), Align2::LEFT_CENTER, "Search all notes", sans(13.0), widgets::lerp_color(MUTED, TEXT2, h));
                    kbd(ui, pos2(r.max.x - 12.0, cy), &format!("{MOD} F"));
                }
                if resp.clicked() {
                    self.actions.push(Action::Search);
                }
                rx -= w + 8.0;
                self.crumbs(ui, rect, x, rx, cy, false);
            }
            Screen::Note(p) if editing => {
                let p = p.clone();
                let w = widgets::pill_width(ui, "Read", true);
                let r = Rect::from_min_max(pos2(rx - w, cy - 20.0), pos2(rx, cy + 20.0));
                let resp = pillbtn(ui, r, "Read", Some(Icon::Read), true);
                widgets::tip(ui, &resp, "Back to reading", &format!("{MOD} E"), false);
                if resp.clicked() {
                    self.actions.push(Action::Read);
                }
                rx -= w + 6.0 + 4.0;
                let (can_undo, can_redo, saved) = self.editor.as_ref().map(|e| (e.can_undo(), e.can_redo(), e.is_saved() && e.dirty_since.is_none())).unwrap_or((false, false, true));
                if ibtn_sized(ui, btn(rx - 22.0), Icon::Redo, 19.0, "Redo", &format!("{MOD} ⇧ Z"), !can_redo).clicked() {
                    self.actions.push(Action::Redo);
                }
                rx -= 48.0;
                if ibtn_sized(ui, btn(rx - 22.0), Icon::Undo, 19.0, "Undo", &format!("{MOD} Z"), !can_undo).clicked() {
                    self.actions.push(Action::Undo);
                }
                widgets::text(ui, pos2(x + 4.0, cy), Align2::LEFT_CENTER, if saved { "Saved" } else { "Editing" }, sans(12.0), MUTED);
                let _ = p;
            }
            Screen::Note(p) => {
                let p = p.clone();
                let pinned = self.tree.find(&p).map(|i| i.pinned).unwrap_or(false);
                let more = btn(rx - 22.0);
                if ibtn(ui, more, Icon::More, "Note options", "", false).clicked() {
                    let items = vec![
                        mi(Icon::Rename, "Rename", "F2", Action::Rename(p.clone())),
                        mi(Icon::Move, "Move to…", "", Action::Move(vec![p.clone()])),
                        mi(Icon::Import, "Show in file manager", "", Action::Reveal(p.clone())),
                        sep(),
                        mi(Icon::Trash, "Delete", "", Action::Delete(vec![p.clone()], true)),
                    ];
                    self.open_menu(ui, MenuAt::Below(more), items);
                }
                rx -= 48.0;
                let pin_icon = if pinned { Icon::PinFill } else { Icon::Pin };
                if ibtn_sized(ui, btn(rx - 22.0), pin_icon, 19.0, if pinned { "Unpin note" } else { "Pin note" }, "", false).clicked() {
                    store::set_pinned(&p, !pinned);
                    self.reload();
                    self.toast(if pinned { "Unpinned" } else { "Pinned" }, ui.input(|i| i.time));
                }
                rx -= 48.0;
                self.crumbs(ui, rect, x, rx, cy, true);
            }
        }
    }

    /// Breadcrumbs. For a note: its folders, small. For a folder: ancestors, then the name large.
    fn crumbs(&mut self, ui: &mut Ui, rect: Rect, x: f32, rx: f32, cy: f32, note: bool) {
        let names: Vec<String> = self
            .stack
            .iter()
            .enumerate()
            .filter_map(|(i, s)| match s {
                Screen::Folder(p) if i == 0 || *p == self.root => Some("Noted".to_string()),
                Screen::Folder(p) => Some(store::file_name(p)),
                Screen::Note(_) => None,
            })
            .collect();
        let narrow = rect.width() < 720.0;
        let clip = Rect::from_min_max(pos2(x, rect.min.y), pos2(rx, rect.max.y));
        let painter = ui.painter().with_clip_rect(clip);
        let ancestors = if note { names.len() } else { names.len().saturating_sub(1) };
        let font = bold(if note { 13.0 } else { 15.0 });
        let mut cx = x;
        if !narrow || note {
            for (i, name) in names.iter().enumerate().take(ancestors) {
                let g = ui.fonts_mut(|f| f.layout_no_wrap(name.clone(), font.clone(), MUTED));
                let r = Rect::from_min_size(pos2(cx, cy - 13.0), vec2(g.size().x + 12.0, 26.0));
                if r.min.x > rx {
                    break;
                }
                let id = ui.id().with(("crumb", i));
                let resp = ui.interact(r.intersect(clip), id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                let h = ui.ctx().animate_bool_with_time(id, resp.hovered(), 0.12);
                if h > 0.0 {
                    painter.rect_filled(r, 8, PILL.gamma_multiply(h));
                }
                painter.galley(pos2(r.min.x + 6.0, cy - g.size().y / 2.0), g, widgets::lerp_color(MUTED, WHITE, h));
                if resp.clicked() {
                    self.actions.push(Action::Crumb(i));
                }
                cx = r.max.x + 6.0;
                let sg = ui.fonts_mut(|f| f.layout_no_wrap("/".into(), xbold(if note { 13.0 } else { 15.0 }), BORDER));
                painter.galley(pos2(cx, cy - sg.size().y / 2.0), sg.clone(), BORDER);
                cx += sg.size().x + 6.0;
            }
        }
        if !note {
            let size = if self.stack.len() == 1 { 28.0 } else { 22.0 };
            let name = names.last().cloned().unwrap_or_default();
            let mut job = widgets::simple_job(&name, xbold(size), WHITE, (rx - cx - 8.0).max(10.0), 1);
            job.wrap.overflow_character = Some('…');
            let g = widgets::layout(ui, job);
            painter.galley(pos2(cx + 4.0, cy - g.size().y / 2.0), g, WHITE);
        }
    }

    pub fn open_menu(&mut self, ui: &Ui, at: MenuAt, items: Vec<MenuItem>) {
        self.menu = Some(Menu { at, items, opened_at: ui.input(|i| i.time), hl: None });
    }

    fn draw_menu(&mut self, ctx: &egui::Context) {
        let Some(menu) = &mut self.menu else { return };
        let screen = ctx.content_rect();
        let h: f32 = menu.items.iter().map(|m| if m.action.is_some() { 40.0 } else { 12.0 }).sum::<f32>() + 12.0;
        let label_w = menu
            .items
            .iter()
            .map(|it| {
                let lw = ctx.fonts_mut(|f| f.layout_no_wrap(it.label.clone(), sans(14.0), WHITE).size().x);
                let kw = if it.key.is_empty() { 0.0 } else { 18.0 + ctx.fonts_mut(|f| f.layout_no_wrap(it.key.clone(), mono(11.0), WHITE).size().x) };
                12.0 + 18.0 + 12.0 + lw + kw + 12.0
            })
            .fold(0.0, f32::max);
        let w = (label_w + 12.0).max(210.0);
        let mut pos = match menu.at {
            MenuAt::Below(r) => pos2(r.max.x - w, r.max.y + 6.0),
            MenuAt::Above(r) => pos2(r.max.x - w, r.min.y - h - 8.0),
            MenuAt::Point(p) => {
                let mut p = p;
                if p.x + w > screen.max.x - 8.0 {
                    p.x = screen.max.x - w - 8.0;
                }
                if p.y + h > screen.max.y - 8.0 {
                    p.y = screen.max.y - h - 8.0;
                }
                p
            }
        };
        pos.x = pos.x.max(screen.min.x + 8.0);
        pos.y = pos.y.max(screen.min.y + 8.0);
        let t = widgets::progress(ctx, menu.opened_at, 0.12);
        let e = widgets::ease_out(t);
        let up = matches!(menu.at, MenuAt::Above(_));
        pos.y += (1.0 - e) * if up { 4.0 } else { -4.0 };
        let mut chosen = None;
        let mut close = false;
        let opened_at = menu.opened_at;
        let mut hover = None;
        let area = egui::Area::new(egui::Id::new("menu")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
            ui.multiply_opacity(e);
            egui::Frame::new().fill(MENU).stroke(Stroke::new(LINE, BORDER)).corner_radius(16).inner_margin(6).show(ui, |ui| {
                ui.set_width(w - 12.0);
                for (i, it) in menu.items.iter().enumerate() {
                    match &it.action {
                        None => {
                            let (r, _) = ui.allocate_exact_size(vec2(w - 12.0, 12.0), Sense::hover());
                            ui.painter().line_segment([pos2(r.min.x + 4.0, r.center().y), pos2(r.max.x - 4.0, r.center().y)], Stroke::new(2.0, DIVIDER));
                        }
                        Some(a) => {
                            let (r, resp) = ui.allocate_exact_size(vec2(w - 12.0, 40.0), Sense::click());
                            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                            if resp.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
                                hover = Some(i);
                            }
                            if resp.hovered() || menu.hl == Some(i) {
                                ui.painter().rect_filled(r, 10, MENU_HOVER);
                            }
                            icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 12.0 + 9.0, r.center().y), vec2(18.0, 18.0)), it.icon, WHITE);
                            ui.painter().text(pos2(r.min.x + 42.0, r.center().y), Align2::LEFT_CENTER, &it.label, sans(14.0), WHITE);
                            if !it.key.is_empty() {
                                ui.painter().text(pos2(r.max.x - 12.0, r.center().y), Align2::RIGHT_CENTER, &it.key, mono(11.0), MUTED);
                            }
                            if resp.clicked() {
                                chosen = Some(a.clone());
                            }
                        }
                    }
                }
            });
        });
        if hover.is_some() {
            menu.hl = hover;
        }
        let now = ctx.input(|i| i.time);
        if ctx.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer() && now - opened_at > 0.05 {
            close = true;
        }
        if let Some(a) = chosen {
            self.actions.push(a);
            close = true;
        }
        if close {
            self.menu = None;
        }
    }

    /* ---------- dialogs ---------- */

    /// A dimmed backdrop that fades in with a panel popping in on top, `w` wide, centred or
    /// `top` px from the top. Returns true when the backdrop outside the panel is clicked.
    fn modal(ctx: &egui::Context, id: &str, alpha: f32, t0: f64, w: f32, top: Option<f32>, add: impl FnOnce(&mut Ui)) -> bool {
        let screen = ctx.content_rect();
        let fade = widgets::progress(ctx, t0, 0.15);
        let t = widgets::progress(ctx, t0, 0.16);
        let e = widgets::ease_pop(t);
        let hid = egui::Id::new((id, "height"));
        let h_prev: f32 = ctx.data(|d| d.get_temp(hid)).unwrap_or(240.0);
        let mut outside = false;
        egui::Area::new(egui::Id::new(id)).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(r, 0, Color32::from_black_alpha((alpha * fade * 255.0) as u8));
            let y = match top {
                Some(t) => screen.min.y + t,
                None => (screen.center().y - h_prev / 2.0).max(screen.min.y + 16.0),
            };
            let rect = Rect::from_min_size(pos2(screen.center().x - w / 2.0, y - 4.0 * (1.0 - e)), vec2(w, screen.height()));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
            child.multiply_opacity(t);
            add(&mut child);
            let used = child.min_rect();
            ctx.data_mut(|d| d.insert_temp(hid, used.height()));
            if resp.clicked() && !resp.interact_pointer_pos().map(|p| used.contains(p)).unwrap_or(false) {
                outside = true;
            }
        });
        outside
    }

    fn draw_dialog(&mut self, ctx: &egui::Context) {
        if self.dialog.is_none() {
            return;
        }
        let Some(dialog) = &mut self.dialog else { return };
        let screen = ctx.content_rect();
        let w = 380.0_f32.min(screen.width() - 32.0);
        let mut close = false;
        let mut action = None;
        let mut msg: Option<String> = None;
        let mut renamed: Option<(PathBuf, PathBuf)> = None;
        let mut created_note: Option<PathBuf> = None;
        let focus = std::mem::take(&mut self.dialog_focus);
        let root = self.root.clone();
        let tree = &self.tree;
        let outside = Self::modal(ctx, "dialog", 0.6, self.dialog_at, w, None, |ui| {
                egui::Frame::new().fill(SURFACE).corner_radius(24).inner_margin(24).show(ui, |ui| {
                    ui.set_width(w - 48.0);
                    let title = |ui: &mut Ui, t: &str| {
                        ui.label(egui::RichText::new(t).font(xbold(16.0)).color(WHITE));
                        ui.add_space(16.0);
                    };
                    let buttons = |ui: &mut Ui, ok: Option<&str>| -> (bool, bool) {
                        ui.add_space(16.0);
                        let (r, _) = ui.allocate_exact_size(vec2(w - 48.0, 40.0), Sense::hover());
                        let mut right = r.max.x;
                        let mut okc = false;
                        if let Some(ok) = ok {
                            let okw = widgets::pill_width(ui, ok, false);
                            let okr = Rect::from_min_max(pos2(right - okw, r.min.y), pos2(right, r.max.y));
                            okc = pillbtn(ui, okr, ok, None, true).clicked();
                            right = okr.min.x - 8.0;
                        }
                        let cw = widgets::pill_width(ui, "Cancel", false);
                        let cancel = pillbtn(ui, Rect::from_min_max(pos2(right - cw, r.min.y), pos2(right, r.max.y)), "Cancel", None, false).clicked();
                        (okc, cancel)
                    };
                    let input = |ui: &mut Ui, value: &mut String| -> bool {
                        let (r, _) = ui.allocate_exact_size(vec2(w - 48.0, 50.0), Sense::hover());
                        let te_id = egui::Id::new("dlginput");
                        let has_focus = ui.ctx().memory(|m| m.has_focus(te_id));
                        ui.painter().rect_stroke(r, 14, Stroke::new(LINE, if has_focus { Color32::from_gray(0x66) } else { BORDER }), StrokeKind::Inside);
                        let resp = ui.put(
                            r.shrink2(vec2(14.0, 15.0)),
                            egui::TextEdit::singleline(value)
                                .id(te_id)
                                .font(sans(15.0))
                                .text_color(WHITE)
                                .frame(egui::Frame::NONE)
                                .margin(egui::Margin::ZERO)
                                .desired_width(f32::INFINITY),
                        );
                        if focus {
                            resp.request_focus();
                            let mut st = egui::TextEdit::load_state(ui.ctx(), te_id).unwrap_or_default();
                            let n = value.chars().count();
                            st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
                            st.store(ui.ctx(), te_id);
                        }
                        resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
                    };
                    match dialog {
                        Dialog::NewNote { dir, value } => {
                            title(ui, "New note");
                            let enter = input(ui, value);
                            let (ok, cancel) = buttons(ui, Some("Create"));
                            if (ok || enter) && !value.trim().is_empty() {
                                match store::create_note(dir, value.trim(), "") {
                                    Ok(p) => created_note = Some(p),
                                    Err(e) => msg = Some(format!("Couldn't create note: {e}")),
                                }
                                close = true;
                            }
                            close |= cancel;
                        }
                        Dialog::NewFolder { dir, value } => {
                            title(ui, "New folder");
                            let enter = input(ui, value);
                            let (ok, cancel) = buttons(ui, Some("Create"));
                            if (ok || enter) && !value.trim().is_empty() {
                                msg = Some(match store::create_folder(dir, value.trim()) {
                                    Ok(_) => "Folder created".into(),
                                    Err(e) => format!("Couldn't create folder: {e}"),
                                });
                                action = Some(Action::ClearSelection);
                                close = true;
                            }
                            close |= cancel;
                        }
                        Dialog::Rename { path, value } => {
                            title(ui, "Rename");
                            let enter = input(ui, value);
                            let (ok, cancel) = buttons(ui, Some("Rename"));
                            if (ok || enter) && !value.trim().is_empty() {
                                match store::rename(path, value.trim()) {
                                    Some(new) => renamed = Some((path.clone(), new)),
                                    None => msg = Some("Something with that name already exists".into()),
                                }
                                close = true;
                            }
                            close |= cancel;
                        }
                        Dialog::Confirm { title: t, body, ok, action: a } => {
                            title(ui, t);
                            let mut job = widgets::simple_job(body, sans(13.0), TEXT2, w - 48.0, 0);
                            job.sections[0].format.line_height = Some(20.0);
                            let g = widgets::layout(ui, job);
                            let (r, _) = ui.allocate_exact_size(vec2(w - 48.0, g.size().y), Sense::hover());
                            ui.painter().galley(r.min, g, TEXT2);
                            let enter = ui.input(|i| i.key_pressed(Key::Enter));
                            let (okc, cancel) = buttons(ui, Some(ok.as_str()));
                            if okc || enter {
                                action = Some(a.clone());
                                close = true;
                            }
                            close |= cancel;
                        }
                        Dialog::Move { paths } => {
                            title(ui, "Move to");
                            let mut folders = Vec::new();
                            tree.all_folders(0, &mut folders);
                            let rows: Vec<(&Item, usize)> = folders
                                .into_iter()
                                .filter(|(f, _)| !paths.iter().any(|p| f.path.starts_with(p)))
                                .filter(|(f, _)| !paths.iter().all(|p| p.parent() == Some(f.path.as_path())))
                                .collect();
                            if rows.is_empty() {
                                ui.label(egui::RichText::new("No other folders yet.").font(sans(13.0)).color(MUTED));
                            }
                            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                                for (f, depth) in rows {
                                    let (r, resp) = ui.allocate_exact_size(vec2(w - 48.0, 42.0), Sense::click());
                                    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                                    if resp.hovered() {
                                        ui.painter().rect_filled(r, 14, TOOLBAR);
                                    }
                                    let x = r.min.x + 12.0 + depth as f32 * 16.0;
                                    icons::paint(ui.painter(), Rect::from_center_size(pos2(x + 8.0, r.center().y), vec2(16.0, 16.0)), Icon::FolderFill, WHITE);
                                    let name = if f.path == root { "Noted (top level)".to_string() } else { f.name.clone() };
                                    ui.painter().text(pos2(x + 26.0, r.center().y), Align2::LEFT_CENTER, name, sans(14.0), WHITE);
                                    if resp.clicked() {
                                        action = Some(Action::MoveTo(paths.clone(), f.path.clone()));
                                        close = true;
                                    }
                                    ui.add_space(2.0);
                                }
                            });
                            let (_, cancel) = buttons(ui, None);
                            close |= cancel;
                        }
                        Dialog::Shortcuts => {
                            title(ui, "Keyboard shortcuts");
                            let rows = [
                                ("Search all notes", format!("{MOD} F")),
                                ("New note", format!("{MOD} N")),
                                ("New folder", format!("{MOD} ⇧ N")),
                                ("Edit / back to reading", format!("E · {MOD} E")),
                                ("Back", "Esc".into()),
                                ("Formatting menu", format!("/ or {MOD} /")),
                                ("Bold · Italic · Link", format!("{MOD} B · I · K")),
                                ("Headings 1–3", format!("{MOD} ⌥ 1–3")),
                                ("Bullet · Numbered · Checkbox", format!("{MOD} ⇧ 8 · 7 · 9")),
                                ("Indent · Outdent", "Tab · ⇧ Tab".into()),
                                ("Undo · Redo", format!("{MOD} Z · {MOD} ⇧ Z")),
                                ("Select several cards", format!("{MOD} click")),
                                ("Rename · Delete card", "F2 · Del".into()),
                            ];
                            for (a, b) in rows {
                                let (r, _) = ui.allocate_exact_size(vec2(w - 48.0, 28.0), Sense::hover());
                                ui.painter().text(pos2(r.min.x, r.center().y), Align2::LEFT_CENTER, a, sans(13.0), TEXT2);
                                kbd(ui, pos2(r.max.x, r.center().y), &b);
                            }
                            ui.add_space(16.0);
                            let (r, _) = ui.allocate_exact_size(vec2(w - 48.0, 40.0), Sense::hover());
                            let dw = widgets::pill_width(ui, "Done", false);
                            if pillbtn(ui, Rect::from_min_max(pos2(r.max.x - dw, r.min.y), r.max), "Done", None, true).clicked() {
                                close = true;
                            }
                        }
                    }
                });
            });
        if close || outside {
            self.dialog = None;
        }
        let now = ctx.input(|i| i.time);
        if let Some((old, new)) = renamed {
            self.remap(&old, &new);
            self.selected.clear();
            self.reload();
        }
        if let Some(p) = created_note {
            self.reload();
            self.editor = None;
            self.open_note_anywhere(&p);
            if let Some(item) = self.tree.find(&p) {
                self.editor = Some(Editor::new(item, None));
            }
        }
        if let Some(m) = msg {
            if m == "Folder created" {
                self.reload();
            }
            self.toast(m, now);
        }
        if let Some(a) = action {
            self.actions.push(a)
        }
    }

    /* ---------- search ---------- */

    fn draw_search(&mut self, ctx: &egui::Context) {
        let Some(opened) = self.search.as_ref().map(|s| s.opened_at) else { return };
        let screen = ctx.content_rect();
        let w = 620.0_f32.min(screen.width() - 32.0);
        let s = self.search.as_mut().unwrap();
        let q = s.query.trim().to_lowercase();
        let mut notes = Vec::new();
        self.tree.all_notes(&mut notes);
        let mut hits: Vec<&Item> = notes.into_iter().filter(|n| q.is_empty() || n.name.to_lowercase().contains(&q) || n.text.to_lowercase().contains(&q)).collect();
        hits.sort_by(|a, b| b.mtime.partial_cmp(&a.mtime).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(30);
        let (up, down, enter) = ctx.input_mut(|i| (i.consume_key(Modifiers::NONE, Key::ArrowUp), i.consume_key(Modifiers::NONE, Key::ArrowDown), i.consume_key(Modifiers::NONE, Key::Enter)));
        if down && !hits.is_empty() {
            s.sel = (s.sel + 1).min(hits.len() - 1)
        }
        if up {
            s.sel = s.sel.saturating_sub(1)
        }
        if s.sel >= hits.len() {
            s.sel = hits.len().saturating_sub(1)
        }
        let mut open = if enter { hits.get(s.sel).map(|r| r.path.clone()) } else { None };
        let root = self.root.clone();
        // Highlight every match of the query.
        let marked = |text: &str, font: egui::FontId, color: Color32, wrap: f32| {
            let mut job = egui::text::LayoutJob::default();
            let f = egui::TextFormat { font_id: font, color, ..Default::default() };
            let low = text.to_lowercase();
            let mut i = 0;
            if !q.is_empty() && low.len() == text.len() {
                while let Some(at) = low[i..].find(&q) {
                    let a = i + at;
                    job.append(&text[i..a], 0.0, f.clone());
                    job.append(&text[a..a + q.len()], 0.0, egui::TextFormat { color: BLACK, background: WHITE, ..f.clone() });
                    i = a + q.len();
                }
            }
            job.append(&text[i..], 0.0, f);
            job.wrap.max_width = wrap;
            job.wrap.max_rows = 1;
            job.wrap.overflow_character = Some('…');
            job
        };
        let outside = Self::modal(ctx, "search", 0.72, opened, w, Some(70.0), |ui| {
                egui::Frame::new().fill(SURFACE).stroke(Stroke::new(LINE, BORDER)).corner_radius(24).show(ui, |ui| {
                    ui.set_width(w);
                    let (r, _) = ui.allocate_exact_size(vec2(w, 56.0), Sense::hover());
                    icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 18.0 + 11.0, r.center().y), vec2(22.0, 22.0)), Icon::Search, MUTED);
                    let kw = kbd(ui, pos2(r.max.x - 18.0, r.center().y), "Esc");
                    let te = ui.put(
                        Rect::from_min_max(pos2(r.min.x + 52.0, r.min.y + 17.0), pos2(r.max.x - 30.0 - kw, r.max.y - 17.0)),
                        egui::TextEdit::singleline(&mut s.query)
                            .id(egui::Id::new("searchinput"))
                            .font(sans(16.0))
                            .text_color(WHITE)
                            .hint_text("Search all notes")
                            .frame(egui::Frame::NONE)
                            .margin(egui::Margin::ZERO)
                            .desired_width(f32::INFINITY),
                    );
                    if s.focus {
                        te.request_focus();
                        s.focus = false;
                    }
                    if te.changed() {
                        s.sel = 0;
                    }
                    ui.painter().line_segment([pos2(r.min.x, r.max.y), pos2(r.max.x, r.max.y)], Stroke::new(2.0, DIVIDER));
                    egui::Frame::new().inner_margin(6).show(ui, |ui| {
                        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                            if hits.is_empty() {
                                let (r, _) = ui.allocate_exact_size(vec2(w - 12.0, 62.0), Sense::hover());
                                ui.painter().text(pos2(r.min.x + 16.0, r.center().y), Align2::LEFT_CENTER, "No matches.", sans(13.0), MUTED);
                            }
                            for (i, n) in hits.iter().enumerate() {
                                let folder = n.path.parent().and_then(|p| p.strip_prefix(&root).ok()).map(|p| p.to_string_lossy().replace('\\', "/")).filter(|s| !s.is_empty());
                                let h = 11.0 + if folder.is_some() { 17.0 } else { 0.0 } + 21.0 + 19.0 + 11.0;
                                let (r, resp) = ui.allocate_exact_size(vec2(w - 12.0, h), Sense::click());
                                let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                                if resp.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
                                    s.sel = i;
                                }
                                if i == s.sel {
                                    ui.painter().rect_filled(r, 16, Color32::from_gray(0x1e));
                                    if down || up {
                                        ui.scroll_to_rect(r, None);
                                    }
                                }
                                let x = r.min.x + 14.0;
                                let mut y = r.min.y + 11.0;
                                if let Some(f) = folder {
                                    ui.painter().text(pos2(x, y), Align2::LEFT_TOP, format!("{f}/"), sans(11.0), MUTED);
                                    y += 17.0;
                                }
                                let tg = widgets::layout(ui, marked(&n.name, xbold(15.0), WHITE, w - 50.0));
                                ui.painter().galley(pos2(x, y), tg, WHITE);
                                y += 21.0;
                                let line = n
                                    .text
                                    .lines()
                                    .find(|l| !q.is_empty() && l.to_lowercase().contains(&q))
                                    .or_else(|| n.text.lines().find(|l| !l.trim().is_empty() && !l.starts_with("# ")))
                                    .unwrap_or("");
                                let c = crate::md::classify(line);
                                let body = if matches!(c.block, crate::md::Block::Plain | crate::md::Block::Rule | crate::md::Block::Fence) { line.trim() } else { &line[c.content.clone()] };
                                let sg = widgets::layout(ui, marked(&crate::md::plain(body), sans(13.0), TEXT3, w - 50.0));
                                ui.painter().galley(pos2(x, y), sg, TEXT3);
                                if resp.clicked() {
                                    open = Some(n.path.clone());
                                }
                            }
                        });
                    });
                });
            });
        if outside {
            self.search = None;
        }
        if let Some(p) = open {
            self.search = None;
            if self.editing() {
                self.close_editor();
            }
            self.open_note_anywhere(&p);
        }
    }

    fn draw_toast(&mut self, ctx: &egui::Context) {
        let Some((msg, t0)) = &mut self.toast else { return };
        let now = ctx.input(|i| i.time);
        if *t0 == 0.0 {
            *t0 = now;
        }
        if now - *t0 > 2.2 {
            self.toast = None;
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        let screen = ctx.content_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("toast")));
        let g = painter.layout_no_wrap(msg.clone(), bold(13.0), BLACK);
        let size = g.size() + vec2(36.0, 20.0);
        let t = widgets::progress(ctx, *t0, 0.22);
        let ease = widgets::ease_pop(t);
        let r = Rect::from_center_size(pos2(screen.center().x, screen.max.y - 96.0 - size.y / 2.0 + (1.0 - ease) * 30.0), size);
        painter.rect_filled(r, CornerRadius::same(20), WHITE.gamma_multiply(t));
        painter.galley(r.min + vec2(18.0, 10.0), g, BLACK.gamma_multiply(t));
    }

    fn drop_files(&mut self, ctx: &egui::Context) {
        let (hovering, dropped) = ctx.input(|i| (!i.raw.hovered_files.is_empty(), i.raw.dropped_files.clone()));
        if hovering {
            let screen = ctx.content_rect().shrink(10.0);
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("drop")));
            painter.rect_filled(screen, 16, Color32::from_black_alpha(205));
            // Dashed border.
            let pts = [screen.left_top(), screen.right_top(), screen.right_bottom(), screen.left_bottom(), screen.left_top()];
            painter.extend(egui::Shape::dashed_line(&pts, Stroke::new(2.0, Color32::from_gray(0x77)), 8.0, 6.0));
            painter.text(screen.center(), Align2::CENTER_CENTER, "Drop .md files to import", xbold(18.0), WHITE);
        }
        if dropped.is_empty() {
            return;
        }
        let dir = self.current_dir();
        let files = dropped.iter().map(|f| f.path().to_path_buf()).collect();
        self.import(&dir, files, ctx.input(|i| i.time));
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Pick up changes made by a sync app when the window comes back into focus.
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if focused && !self.was_focused && !self.editing() && self.drag.is_none() && self.dialog.is_none() && self.menu.is_none() {
            self.reload();
        }
        if !focused && self.was_focused {
            self.save_editor();
        }
        self.was_focused = focused;
        if ctx.input(|i| i.viewport().close_requested()) {
            self.save_editor();
        }

        self.global_keys(&ctx);
        let overlay = self.menu.is_some() || self.dialog.is_some() || self.search.is_some() || self.drag.is_some();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("overlay-open"), overlay));

        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0, BLACK);
        let tbar = Rect::from_min_size(full.min, vec2(full.width(), TITLEBAR));
        self.titlebar(ui, tbar);

        // Each screen change fades the new view in and lifts it 8px.
        let key = format!("{:?}{}", self.top(), self.editing());
        let now = ctx.input(|i| i.time);
        if key != self.view_key {
            if !self.view_key.is_empty() {
                self.view_at = now;
            }
            self.view_key = key;
            self.menu = None;
        }
        let t = widgets::progress(&ctx, self.view_at, 0.22);
        let ease = widgets::ease_out(t);
        let dy = (1.0 - ease) * 8.0;
        let stage = Rect::from_min_max(pos2(full.min.x, tbar.max.y), full.max);
        let mut view = ui.new_child(egui::UiBuilder::new().max_rect(stage).id_salt("view"));
        view.set_clip_rect(stage);
        view.multiply_opacity(ease);
        let content = stage.translate(vec2(0.0, dy));
        let hdr = Rect::from_min_size(content.min, vec2(content.width(), HEADER));
        let body = Rect::from_min_max(pos2(content.min.x, hdr.max.y), pos2(content.max.x, content.max.y));

        let top = self.top().clone();
        let mut body_ui = view.new_child(egui::UiBuilder::new().max_rect(body).id_salt(("body", format!("{top:?}"), self.editing())));
        body_ui.set_clip_rect(body.intersect(stage));
        match &top {
            Screen::Folder(dir) => crate::grid::show(self, &mut body_ui, dir.clone()),
            Screen::Note(p) => {
                if self.editing() {
                    crate::editor::show(self, &mut body_ui)
                } else {
                    crate::reader::show(self, &mut body_ui, p.clone())
                }
            }
        }
        let mut hdr_ui = view.new_child(egui::UiBuilder::new().max_rect(hdr).id_salt("header"));
        self.header(&mut hdr_ui, hdr);

        self.drop_files(&ctx);
        self.draw_menu(&ctx);
        self.draw_dialog(&ctx);
        self.draw_search(&ctx);
        self.draw_toast(&ctx);
        self.resize_edges(&ctx);

        // Autosave shortly after typing stops.
        if let Some(e) = &self.editor {
            if e.dirty_since.map(|t| now - t > 0.6).unwrap_or(false) {
                if let Some(e) = &mut self.editor {
                    if let Err(msg) = e.write_text() {
                        self.toast = Some((msg, now));
                    }
                }
            } else if e.dirty_since.is_some() {
                ctx.request_repaint_after(std::time::Duration::from_millis(150));
            }
        }

        for a in std::mem::take(&mut self.actions) {
            self.run(&ctx, a);
        }
    }
}
