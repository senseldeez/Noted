//! App state, navigation, the header, and the overlays (menus, dialogs, search, toasts).
use crate::editor::Editor;
use crate::icons::{self, Icon};
use crate::store::{self, Item};
use crate::theme::*;
use crate::widgets::{self, ibtn, kbd, pillbtn, MOD};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, Key, Modifiers, Rect, Sense, Stroke, StrokeKind, Ui};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone, PartialEq, Debug)]
pub enum Screen {
    Folder(PathBuf),
    Note(PathBuf),
}

#[derive(Clone, Debug)]
pub enum Action {
    OpenFolder(PathBuf),
    OpenNote(PathBuf),
    EditNote(PathBuf),
    Read,
    Back,
    Crumb(usize),
    NewNote(PathBuf),
    NewFolder(PathBuf),
    Rename(PathBuf),
    Move(Vec<PathBuf>),
    MoveTo(Vec<PathBuf>, PathBuf),
    Trash(Vec<PathBuf>),
    Pin(Vec<PathBuf>, bool),
    Reveal(PathBuf),
    Refresh,
    ChooseRoot,
    Shortcuts,
    Search,
    Reorder { dir: PathBuf, pinned: Vec<String>, order: Vec<String> },
    WriteNote(PathBuf, String),
    ClearSelection,
}

pub struct MenuItem {
    pub icon: Icon,
    pub label: String,
    pub key: &'static str,
    pub action: Option<Action>,
}

pub fn mi(icon: Icon, label: &str, key: &'static str, action: Action) -> MenuItem {
    MenuItem { icon, label: label.into(), key, action: Some(action) }
}
pub fn sep() -> MenuItem {
    MenuItem { icon: Icon::Close, label: String::new(), key: "", action: None }
}

pub struct Menu {
    pub pos: egui::Pos2,
    pub items: Vec<MenuItem>,
    pub opened_at: f64,
}

pub enum Dialog {
    NewFolder { dir: PathBuf, value: String },
    Rename { path: PathBuf, value: String },
    Move { paths: Vec<PathBuf> },
    Shortcuts,
}

pub struct Search {
    pub query: String,
    pub sel: usize,
    pub focus: bool,
}

pub struct Drag {
    pub path: PathBuf,
    pub grab: egui::Vec2,
    pub pinned: bool,
    /// Current order of the dragged card's group while dragging.
    pub order: Vec<PathBuf>,
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
    pub search: Option<Search>,
    pub toast: Option<(String, f64)>,
    pub folds: HashMap<PathBuf, HashSet<usize>>,
    pub actions: Vec<Action>,
    was_focused: bool,
    dialog_focus: bool,
    pub top_inset: f32,
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
            search: None,
            toast: None,
            folds: HashMap::new(),
            actions: vec![],
            was_focused: true,
            dialog_focus: false,
            top_inset: if cfg!(target_os = "macos") { 28.0 } else { 0.0 },
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
            let old = e.path.clone();
            match e.save() {
                Ok(Some(new)) => {
                    for s in &mut self.stack {
                        if let Screen::Note(p) = s {
                            if *p == old {
                                *p = new.clone()
                            }
                        }
                    }
                }
                Ok(None) => {}
                Err(msg) => self.toast = Some((msg, 0.0)),
            }
        }
    }

    fn close_editor(&mut self) {
        self.save_editor();
        self.editor = None;
        self.reload();
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
    }

    /// Navigate to a note from anywhere (search results).
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
            Action::EditNote(p) => {
                self.selected.clear();
                if !matches!(self.top(), Screen::Note(x) if *x == p) {
                    self.stack.push(Screen::Note(p.clone()));
                }
                if let Some(item) = self.tree.find(&p) {
                    self.editor = Some(Editor::new(item));
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
            Action::Crumb(i) => {
                if self.editing() {
                    self.close_editor()
                }
                self.selected.clear();
                self.stack.truncate(i + 1);
            }
            Action::NewNote(dir) => {
                if self.editing() {
                    self.close_editor()
                }
                match store::create_note(&dir, "Untitled", "") {
                    Ok(p) => {
                        self.reload();
                        if matches!(self.top(), Screen::Note(_)) {
                            self.stack.pop();
                        }
                        self.stack.push(Screen::Note(p.clone()));
                        if let Some(item) = self.tree.find(&p) {
                            let mut e = Editor::new(item);
                            e.focus_title = true;
                            e.is_new = true;
                            self.editor = Some(e);
                        }
                    }
                    Err(e) => self.toast(format!("Couldn't create note: {e}"), now),
                }
            }
            Action::NewFolder(dir) => {
                self.dialog = Some(Dialog::NewFolder { dir, value: String::new() });
                self.dialog_focus = true;
            }
            Action::Rename(p) => {
                let value = self.tree.find(&p).map(|i| i.name.clone()).unwrap_or_default();
                self.dialog = Some(Dialog::Rename { path: p, value });
                self.dialog_focus = true;
            }
            Action::Move(paths) => self.dialog = Some(Dialog::Move { paths }),
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
            Action::Trash(paths) => {
                if let Some(e) = &self.editor {
                    if paths.iter().any(|p| e.path.starts_with(p)) {
                        self.editor = None;
                    }
                }
                let n = paths.iter().filter(|p| store::trash(p)).count();
                self.selected.clear();
                self.reload();
                self.toast(
                    if n == 1 { "Moved to trash".into() } else if n == 0 { "Couldn't delete".to_string() } else { format!("Moved {n} items to trash") },
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
            Action::Shortcuts => self.dialog = Some(Dialog::Shortcuts),
            Action::Search => {
                self.save_editor();
                self.search = Some(Search { query: String::new(), sel: 0, focus: true })
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
        }
    }

    /* ---------- keys ---------- */

    fn global_keys(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();
        let cmd = Modifiers::COMMAND;
                let (f, k, n, sn, e) = ctx.input_mut(|i| {
            (
                i.consume_key(cmd, Key::F),
                i.consume_key(cmd, Key::K) && !typing,
                i.consume_key(cmd, Key::N),
                i.consume_key(cmd | Modifiers::SHIFT, Key::N),
                i.consume_key(cmd, Key::E),
            )
        });
        let dir = self.current_dir();
        if f || k {
            self.actions.push(Action::Search)
        }
        if sn {
            self.actions.push(Action::NewFolder(dir.clone()))
        } else if n {
            self.actions.push(Action::NewNote(dir.clone()))
        }
        if e {
            match self.top().clone() {
                Screen::Note(p) if !self.editing() => self.actions.push(Action::EditNote(p)),
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
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.menu = None;
            }
            return;
        }
        if self.editing() {
            if self.editor.as_ref().map(|e| e.palette.is_none()).unwrap_or(true)
                && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
            {
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
                self.actions.push(Action::EditNote(p.clone()))
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
                self.actions.push(Action::Trash(sel))
            }
        }
        if q {
            self.actions.push(Action::Shortcuts)
        }
    }

    /* ---------- header ---------- */

    /// Header across the top: back, breadcrumbs, then `right` items laid out right to left.
    fn header(&mut self, ui: &mut Ui, rect: Rect) {
        let depth = self.stack.len();
        let is_root = depth == 1;
        let mut x = rect.min.x + if is_root { 26.0 } else { 10.0 };
        let cy = rect.min.y + 14.0 + 22.0;
        // Window dragging on macOS where the title bar is hidden.
        if cfg!(target_os = "macos") {
            let drag = ui.interact(rect, ui.id().with("titledrag"), Sense::drag());
            if drag.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        }
        if !is_root {
            if ibtn(ui, Rect::from_center_size(pos2(x + 22.0, cy), vec2(44.0, 44.0)), Icon::Back, "Back (Esc)", false).clicked() {
                self.actions.push(Action::Back);
            }
            x += 48.0;
        }

        // Right side.
        let mut rx = rect.max.x - 18.0;
        let top = self.top().clone();
        let editing = self.editing();
        match &top {
            Screen::Folder(dir) => {
                if ibtn(ui, Rect::from_center_size(pos2(rx - 22.0, cy), vec2(44.0, 44.0)), Icon::More, "More", false).clicked() {
                    let mut items = vec![
                        mi(Icon::Refresh, "Refresh", "", Action::Refresh),
                        mi(Icon::Folder, "Change notes folder", "", Action::ChooseRoot),
                        mi(Icon::Reveal, "Show in file manager", "", Action::Reveal(dir.clone())),
                    ];
                    if dir != &self.root {
                        items.insert(0, sep());
                        items.insert(0, mi(Icon::Rename, "Rename folder", "F2", Action::Rename(dir.clone())));
                    }
                    items.push(sep());
                    items.push(mi(Icon::Keys, "Keyboard shortcuts", "?", Action::Shortcuts));
                    self.open_menu(ui, pos2(rx - 220.0, cy + 26.0), items);
                }
                rx -= 48.0;
                if ibtn(ui, Rect::from_center_size(pos2(rx - 22.0, cy), vec2(44.0, 44.0)), Icon::FolderPlus, "New folder", false).clicked() {
                    self.actions.push(Action::NewFolder(dir.clone()));
                }
                rx -= 52.0;
                // Search pill.
                let w = 230.0_f32.min((rx - x - 200.0).max(44.0));
                let r = Rect::from_min_max(pos2(rx - w, cy - 20.0), pos2(rx, cy + 20.0));
                let resp = ui.interact(r, ui.id().with("searchpill"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                let col = if resp.hovered() { Color32::from_gray(0x55) } else { BORDER };
                ui.painter().rect_stroke(r, 20, Stroke::new(LINE, col), StrokeKind::Inside);
                icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 22.0, cy), vec2(17.0, 17.0)), Icon::Search, MUTED);
                if w > 150.0 {
                    widgets::text(ui, pos2(r.min.x + 40.0, cy), Align2::LEFT_CENTER, "Search notes", sans(13.0), if resp.hovered() { TEXT2 } else { MUTED });
                    kbd(ui, pos2(r.max.x - 12.0, cy), &format!("{MOD} F"));
                }
                if resp.clicked() {
                    self.actions.push(Action::Search);
                }
                rx -= w + 12.0;
            }
            Screen::Note(p) => {
                let p = p.clone();
                let pinned = self.tree.find(&p).map(|i| i.pinned).unwrap_or(false);
                if ibtn(ui, Rect::from_center_size(pos2(rx - 22.0, cy), vec2(44.0, 44.0)), Icon::More, "More", false).clicked() {
                    let items = vec![
                        mi(Icon::Pin, if pinned { "Unpin" } else { "Pin" }, "", Action::Pin(vec![p.clone()], !pinned)),
                        mi(Icon::Rename, "Rename", "F2", Action::Rename(p.clone())),
                        mi(Icon::Move, "Move to…", "", Action::Move(vec![p.clone()])),
                        mi(Icon::Reveal, "Show in file manager", "", Action::Reveal(p.clone())),
                        sep(),
                        mi(Icon::Trash, "Delete", "", Action::Trash(vec![p.clone()])),
                    ];
                    self.open_menu(ui, pos2(rx - 220.0, cy + 26.0), items);
                }
                rx -= 52.0;
                let (label, icon) = if editing { ("Read", Icon::Read) } else { ("Edit", Icon::Edit) };
                let w = widgets::pill_width(ui, label, true);
                let r = Rect::from_min_max(pos2(rx - w, cy - 20.0), pos2(rx, cy + 20.0));
                if pillbtn(ui, r, label, Some(icon), !editing).on_hover_text(format!("{MOD} E")).clicked() {
                    self.actions.push(if editing { Action::Read } else { Action::EditNote(p.clone()) });
                }
                if pinned {
                    icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x - 20.0, cy), vec2(16.0, 16.0)), Icon::Pin, MUTED);
                }
                rx -= w + 30.0;
            }
        }

        // Breadcrumbs.
        let names: Vec<String> = self
            .stack
            .iter()
            .enumerate()
            .map(|(i, s)| match s {
                Screen::Folder(p) if i == 0 || *p == self.root => "Noted".to_string(),
                Screen::Folder(p) => store::file_name(p),
                Screen::Note(_) => String::new(),
            })
            .collect();
        let folder_count = self.stack.iter().filter(|s| matches!(s, Screen::Folder(_))).count();
        let is_note = matches!(top, Screen::Note(_));
        let avail = (rx - x).max(40.0);
        let clip = Rect::from_min_max(pos2(x, rect.min.y), pos2(rx, rect.max.y));
        let painter = ui.painter().with_clip_rect(clip);
        // Ancestors (all folders except the current one; for notes, all folders).
        let ancestors = if is_note { folder_count } else { folder_count - 1 };
        let mut cx = x;
        for (i, name) in names.iter().enumerate().take(ancestors) {
            let g = ui.fonts_mut(|f| f.layout_no_wrap(name.clone(), bold(15.0), MUTED));
            let r = Rect::from_min_size(pos2(cx, cy - 13.0), vec2(g.size().x + 12.0, 26.0));
            if r.min.x > rx {
                break;
            }
            let resp = ui.interact(r.intersect(clip), ui.id().with(("crumb", i)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.hovered() {
                painter.rect_filled(r, 8, PILL);
            }
            painter.galley(pos2(r.min.x + 6.0, cy - g.size().y / 2.0), g, if resp.hovered() { WHITE } else { MUTED });
            if resp.clicked() {
                self.actions.push(Action::Crumb(i));
            }
            cx = r.max.x + 2.0;
            if !(is_note && i + 1 == ancestors) {
                painter.text(pos2(cx + 4.0, cy), Align2::LEFT_CENTER, "/", xbold(15.0), BORDER);
                cx += 14.0;
            }
        }
        if !is_note {
            let size = if is_root { 28.0 } else { 22.0 };
            let name = names[folder_count - 1].clone();
            let mut job = widgets::simple_job(&name, xbold(size), WHITE, (rx - cx - 8.0).max(10.0), 1);
            job.wrap.overflow_character = Some('…');
            let g = widgets::layout(ui, job);
            painter.galley(pos2(cx + 4.0, cy - g.size().y / 2.0), g, WHITE);
        }
        let _ = avail;
    }

    pub fn open_menu(&mut self, ui: &Ui, pos: egui::Pos2, items: Vec<MenuItem>) {
        self.menu = Some(Menu { pos, items, opened_at: ui.input(|i| i.time) });
    }

    fn draw_menu(&mut self, ctx: &egui::Context) {
        let Some(menu) = &self.menu else { return };
        let screen = ctx.content_rect();
        let h: f32 = menu.items.iter().map(|m| if m.action.is_some() { 40.0 } else { 12.0 }).sum::<f32>() + 12.0;
        let w = 230.0;
        let mut pos = menu.pos;
        pos.x = pos.x.clamp(8.0, screen.max.x - w - 8.0);
        if pos.y + h > screen.max.y - 8.0 {
            pos.y = (pos.y - h).max(8.0);
        }
        let mut chosen = None;
        let mut close = false;
        let opened_at = menu.opened_at;
        let area = egui::Area::new(egui::Id::new("menu")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
            egui::Frame::new()
                .fill(MENU)
                .stroke(Stroke::new(LINE, BORDER))
                .corner_radius(16)
                .inner_margin(6)
                .show(ui, |ui| {
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
                                if resp.hovered() {
                                    ui.painter().rect_filled(r, 10, MENU_HOVER);
                                }
                                let danger = it.icon == Icon::Trash;
                                let col = if danger { Color32::from_rgb(0xff, 0x8a, 0x80) } else { WHITE };
                                icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 21.0, r.center().y), vec2(18.0, 18.0)), it.icon, col);
                                ui.painter().text(pos2(r.min.x + 42.0, r.center().y), Align2::LEFT_CENTER, &it.label, sans(14.0), col);
                                if !it.key.is_empty() {
                                    ui.painter().text(pos2(r.max.x - 12.0, r.center().y), Align2::RIGHT_CENTER, it.key, mono(11.0), MUTED);
                                }
                                if resp.clicked() {
                                    chosen = Some(a.clone());
                                }
                                let _ = i;
                            }
                        }
                    }
                });
        });
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

    fn scrim(ctx: &egui::Context, id: &str, alpha: u8) -> bool {
        let screen = ctx.content_rect();
        let mut clicked = false;
        egui::Area::new(egui::Id::new(id)).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(r, 0, Color32::from_black_alpha(alpha));
            clicked = resp.clicked();
        });
        clicked
    }

    fn draw_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.dialog else { return };
        if Self::scrim(ctx, "dlgscrim", 150) {
            self.dialog = None;
            return;
        }
        let screen = ctx.content_rect();
        let w = 380.0_f32.min(screen.width() - 32.0);
        let mut close = false;
        let mut action = None;
        let focus = std::mem::take(&mut self.dialog_focus);
        let root = self.root.clone();
        let tree = &self.tree;
        egui::Area::new(egui::Id::new("dialog"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, -20.0))
            .show(ctx, |ui| {
                egui::Frame::new().fill(SURFACE).corner_radius(24).inner_margin(24).show(ui, |ui| {
                    ui.set_width(w - 48.0);
                    let title = |ui: &mut Ui, t: &str| {
                        ui.label(egui::RichText::new(t).font(xbold(16.0)).color(WHITE));
                        ui.add_space(16.0);
                    };
                    let buttons = |ui: &mut Ui, ok: &str| -> (bool, bool) {
                        ui.add_space(16.0);
                        let (r, _) = ui.allocate_exact_size(vec2(w - 48.0, 40.0), Sense::hover());
                        let okw = widgets::pill_width(ui, ok, false);
                        let cw = widgets::pill_width(ui, "Cancel", false);
                        let okr = Rect::from_min_max(pos2(r.max.x - okw, r.min.y), r.max);
                        let cr = Rect::from_min_max(pos2(okr.min.x - 8.0 - cw, r.min.y), pos2(okr.min.x - 8.0, r.max.y));
                        let ok = pillbtn(ui, okr, ok, None, true).clicked();
                        let cancel = pillbtn(ui, cr, "Cancel", None, false).clicked();
                        (ok, cancel)
                    };
                    let input = |ui: &mut Ui, value: &mut String, hint: &str| -> bool {
                        let r = egui::Frame::new().stroke(Stroke::new(LINE, BORDER)).corner_radius(14).inner_margin(14).show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::singleline(value)
                                    .id(egui::Id::new("dlginput"))
                                    .font(sans(15.0))
                                    .text_color(WHITE)
                                    .hint_text(hint)
                                    .frame(egui::Frame::NONE)
                                    .desired_width(f32::INFINITY),
                            )
                        });
                        if focus {
                            r.inner.request_focus();
                            if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), r.inner.id) {
                                let n = value.chars().count();
                                st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
                                st.store(ui.ctx(), r.inner.id);
                            }
                        }
                        r.inner.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
                    };
                    match dialog {
                        Dialog::NewFolder { dir, value } => {
                            title(ui, "New folder");
                            let enter = input(ui, value, "Folder name");
                            let (ok, cancel) = buttons(ui, "Create");
                            if (ok || enter) && !value.trim().is_empty() {
                                match store::create_folder(dir, value) {
                                    Ok(p) => action = Some(Action::OpenFolder(p)),
                                    Err(_) => {}
                                }
                                close = true;
                            }
                            close |= cancel;
                        }
                        Dialog::Rename { path, value } => {
                            title(ui, "Rename");
                            let enter = input(ui, value, "Name");
                            let (ok, cancel) = buttons(ui, "Rename");
                            if (ok || enter) && !value.trim().is_empty() {
                                action = Some(Action::Refresh);
                                if let Some(new) = store::rename(path, value) {
                                    action = Some(Action::MoveTo(vec![], new.clone()));
                                    // Encode a rename as remap via a dedicated path pair.
                                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("renamed"), (path.clone(), new)));
                                } else {
                                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("rename_taken"), true));
                                }
                                close = true;
                            }
                            close |= cancel;
                        }
                        Dialog::Move { paths } => {
                            title(ui, if paths.len() == 1 { "Move to" } else { "Move items to" });
                            let mut folders = Vec::new();
                            tree.all_folders(0, &mut folders);
                            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                                for (f, depth) in folders {
                                    if paths.iter().any(|p| f.path.starts_with(p)) {
                                        continue;
                                    }
                                    let (r, resp) = ui.allocate_exact_size(vec2(w - 48.0, 42.0), Sense::click());
                                    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                                    if resp.hovered() {
                                        ui.painter().rect_filled(r, 14, TOOLBAR);
                                    }
                                    let x = r.min.x + 12.0 + depth as f32 * 18.0;
                                    icons::paint(ui.painter(), Rect::from_center_size(pos2(x + 8.0, r.center().y), vec2(16.0, 16.0)), Icon::Folder, MUTED);
                                    let name = if f.path == root { "Noted".to_string() } else { f.name.clone() };
                                    ui.painter().text(pos2(x + 26.0, r.center().y), Align2::LEFT_CENTER, name, sans(14.0), WHITE);
                                    if resp.clicked() {
                                        action = Some(Action::MoveTo(paths.clone(), f.path.clone()));
                                        close = true;
                                    }
                                }
                            });
                            let (_, cancel) = {
                                ui.add_space(12.0);
                                let (r, _) = ui.allocate_exact_size(vec2(w - 48.0, 40.0), Sense::hover());
                                let cw = widgets::pill_width(ui, "Cancel", false);
                                (false, pillbtn(ui, Rect::from_min_max(pos2(r.max.x - cw, r.min.y), r.max), "Cancel", None, false).clicked())
                            };
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
        if close {
            self.dialog = None;
        }
        // Renames are applied here so open screens follow the new path.
        if let Some((old, new)) = ctx.data_mut(|d| d.remove_temp::<(PathBuf, PathBuf)>(egui::Id::new("renamed"))) {
            self.remap(&old, &new);
            if self.selected.remove(&old) {
                self.selected.insert(new.clone());
            }
            self.reload();
            action = None;
        }
        if ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::new("rename_taken"))).is_some() {
            self.toast("That name is taken", ctx.input(|i| i.time));
            action = None;
        }
        if let Some(a) = action {
            self.actions.push(a)
        }
    }

    /* ---------- search ---------- */

    fn draw_search(&mut self, ctx: &egui::Context) {
        if self.search.is_none() {
            return;
        }
        if Self::scrim(ctx, "searchscrim", 185) {
            self.search = None;
            return;
        }
        let screen = ctx.content_rect();
        let w = 620.0_f32.min(screen.width() - 32.0);
        let s = self.search.as_mut().unwrap();
        let q = s.query.trim().to_lowercase();
        let mut notes = Vec::new();
        self.tree.all_notes(&mut notes);
        let mut results: Vec<(&Item, Option<(String, usize, usize)>, i32)> = Vec::new();
        if q.is_empty() {
            let mut v: Vec<&Item> = notes.clone();
            v.sort_by(|a, b| b.mtime.partial_cmp(&a.mtime).unwrap_or(std::cmp::Ordering::Equal));
            results = v.into_iter().take(12).map(|n| (n, None, 0)).collect();
        } else {
            for n in notes {
                let in_name = n.name.to_lowercase().contains(&q);
                let mut snip = None;
                for line in n.text.lines() {
                    let pl = crate::md::plain(line);
                    let low = pl.to_lowercase();
                    if let Some(at) = low.find(&q) {
                        // Only use the match when lowercase keeps byte offsets (ASCII-ish text).
                        if low.len() == pl.len() {
                            let start = pl[..at].char_indices().rev().nth(40).map(|(i, _)| i).unwrap_or(0);
                            let s2 = pl[start..].to_string();
                            snip = Some((s2, at - start, q.len()));
                        } else {
                            snip = Some((pl.clone(), 0, 0));
                        }
                        break;
                    }
                }
                if in_name || snip.is_some() {
                    results.push((n, snip, if in_name { 0 } else { 1 }));
                }
            }
            results.sort_by_key(|r| r.2);
            results.truncate(50);
        }
        let (up, down, enter) = ctx.input_mut(|i| {
            (i.consume_key(Modifiers::NONE, Key::ArrowUp), i.consume_key(Modifiers::NONE, Key::ArrowDown), i.consume_key(Modifiers::NONE, Key::Enter))
        });
        if down && !results.is_empty() {
            s.sel = (s.sel + 1).min(results.len() - 1)
        }
        if up {
            s.sel = s.sel.saturating_sub(1)
        }
        if s.sel >= results.len() {
            s.sel = results.len().saturating_sub(1)
        }
        let mut open = if enter { results.get(s.sel).map(|r| r.0.path.clone()) } else { None };
        let root = self.root.clone();
        egui::Area::new(egui::Id::new("search"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos2(screen.center().x - w / 2.0, screen.min.y + 70.0))
            .show(ctx, |ui| {
                egui::Frame::new().fill(SURFACE).stroke(Stroke::new(LINE, BORDER)).corner_radius(24).show(ui, |ui| {
                    ui.set_width(w);
                    let (r, _) = ui.allocate_exact_size(vec2(w, 56.0), Sense::hover());
                    icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 28.0, r.center().y), vec2(20.0, 20.0)), Icon::Search, MUTED);
                    let te = ui.put(
                        Rect::from_min_max(pos2(r.min.x + 50.0, r.min.y + 16.0), pos2(r.max.x - 18.0, r.max.y - 16.0)),
                        egui::TextEdit::singleline(&mut s.query)
                            .id(egui::Id::new("searchinput"))
                            .font(sans(16.0))
                            .text_color(WHITE)
                            .hint_text("Search all notes")
                            .frame(egui::Frame::NONE)
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
                            if results.is_empty() {
                                let (r, _) = ui.allocate_exact_size(vec2(w - 12.0, 60.0), Sense::hover());
                                ui.painter().text(pos2(r.min.x + 16.0, r.center().y), Align2::LEFT_CENTER, "No notes match", sans(13.0), MUTED);
                            }
                            for (i, (n, snip, _)) in results.iter().enumerate() {
                                let h = if snip.is_some() || q.is_empty() { 70.0 } else { 52.0 };
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
                                let folder = n
                                    .path
                                    .parent()
                                    .and_then(|p| p.strip_prefix(&root).ok())
                                    .map(|p| p.to_string_lossy().replace('\\', " / ").replace('/', " / "))
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or_else(|| "Noted".into());
                                let x = r.min.x + 14.0;
                                let mut y = r.min.y + 11.0;
                                ui.painter().text(pos2(x, y), Align2::LEFT_TOP, folder, sans(11.0), MUTED);
                                y += 16.0;
                                ui.painter().text(pos2(x, y), Align2::LEFT_TOP, &n.name, xbold(15.0), WHITE);
                                y += 21.0;
                                let snippet = match snip {
                                    Some(s) => Some(s.clone()),
                                    None if q.is_empty() => n.text.lines().map(crate::md::plain).find(|l| !l.trim().is_empty()).map(|l| (l, 0, 0)),
                                    None => None,
                                };
                                if let Some((text, at, len)) = snippet {
                                    let mut job = egui::text::LayoutJob::default();
                                    let f = egui::TextFormat { font_id: sans(13.0), color: TEXT3, ..Default::default() };
                                    let end = (at + len).min(text.len());
                                    job.append(&text[..at.min(text.len())], 0.0, f.clone());
                                    job.append(&text[at.min(text.len())..end], 0.0, egui::TextFormat { color: BLACK, background: WHITE, ..f.clone() });
                                    job.append(&text[end..], 0.0, f);
                                    job.wrap.max_width = w - 50.0;
                                    job.wrap.max_rows = 1;
                                    let g = widgets::layout(ui, job);
                                    ui.painter().galley(pos2(x, y), g, TEXT3);
                                }
                                if resp.clicked() {
                                    open = Some(n.path.clone());
                                }
                            }
                        });
                    });
                });
            });
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
        let t = ((now - *t0) as f32 / 0.22).min(1.0);
        let ease = 1.0 - (1.0 - t).powi(3);
        let r = Rect::from_center_size(pos2(screen.center().x, screen.max.y - 96.0 - size.y / 2.0 + (1.0 - ease) * 20.0), size);
        painter.rect_filled(r, CornerRadius::same(20), WHITE.gamma_multiply(ease));
        painter.galley(r.min + vec2(18.0, 10.0), g, BLACK);
    }

    fn drop_files(&mut self, ctx: &egui::Context) {
        let (hovering, dropped) = ctx.input(|i| (!i.raw.hovered_files.is_empty(), i.raw.dropped_files.clone()));
        if hovering {
            let screen = ctx.content_rect().shrink(10.0);
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("drop")));
            painter.rect_filled(screen, 16, Color32::from_black_alpha(205));
            painter.rect_stroke(screen, 16, Stroke::new(2.0, Color32::from_gray(0x77)), StrokeKind::Inside);
            painter.text(screen.center(), Align2::CENTER_CENTER, "Drop .md files to import", xbold(18.0), WHITE);
        }
        if dropped.is_empty() {
            return;
        }
        let dir = self.current_dir();
        let (mut ok, mut skipped) = (0, 0);
        for f in dropped {
            let p = f.path().to_path_buf();
            if p.as_os_str().is_empty() {
                skipped += 1;
                continue;
            }
            let name = store::file_name(&p);
            if !store::is_note(&name) {
                skipped += 1;
                continue;
            }
            match std::fs::read_to_string(&p).ok().and_then(|t| store::create_note(&dir, &name, &t).ok()) {
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
        self.toast(msg, ctx.input(|i| i.time));
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Pick up changes made by a sync app when the window comes back into focus.
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if focused && !self.was_focused && !self.editing() && self.drag.is_none() {
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

        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0, BLACK);
        let content = Rect::from_min_max(pos2(full.min.x, full.min.y + self.top_inset), full.max);
        let hdr = Rect::from_min_size(content.min, vec2(content.width(), 66.0));
        let body = Rect::from_min_max(pos2(content.min.x, hdr.max.y), content.max);

        let top = self.top().clone();
        let mut body_ui = ui.new_child(egui::UiBuilder::new().max_rect(body).id_salt(("body", format!("{top:?}"), self.editing())));
        body_ui.set_clip_rect(body);
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
        self.header(ui, hdr);

        self.drop_files(&ctx);
        self.draw_menu(&ctx);
        self.draw_dialog(&ctx);
        self.draw_search(&ctx);
        self.draw_toast(&ctx);

        // Autosave while typing.
        let now = ctx.input(|i| i.time);
        if let Some(e) = &self.editor {
            if e.dirty_since.map(|t| now - t > 1.0).unwrap_or(false) {
                if let Some(e) = &mut self.editor {
                    if let Err(msg) = e.write_text() {
                        self.toast = Some((msg, now));
                    }
                }
            } else if e.dirty_since.is_some() {
                ctx.request_repaint_after(std::time::Duration::from_millis(300));
            }
        }

        for a in std::mem::take(&mut self.actions) {
            self.run(&ctx, a);
        }
    }
}
