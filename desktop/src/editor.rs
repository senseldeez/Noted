//! Live-preview editor: markdown is styled in place, syntax only shows on the line being
//! edited, which sits on a rounded grey pill. Slash menu, toolbar and list shortcuts.
use crate::app::App;
use crate::icons::{self, Icon};
use crate::blocks;
use crate::md::{self, Block};
use crate::reader::{self, format_for};
use crate::store::{self, Item};
use crate::theme::*;
use crate::widgets::{self, MOD};
use egui::text::{CCursor, CCursorRange, LayoutJob};
use egui::{pos2, vec2, Align2, Color32, FontId, Id, Key, Modifiers, Rect, Sense, Shape, Stroke, StrokeKind, TextFormat, Ui};
use std::collections::HashSet;
use std::sync::Arc;
use egui::Galley;
use crate::hang;
use std::path::PathBuf;

const LH: f32 = 28.0;

pub enum PalMode {
    /// Opened by typing `/`; the char index of that slash. The query is the text after it.
    Slash(usize),
    /// Opened with Ctrl+/ or the toolbar: its own search field, applied to `at` when chosen.
    Input { query: String, at: (usize, usize), focus: bool },
}

pub struct Palette {
    pub mode: PalMode,
    pub sel: usize,
    lastq: String,
    opened: f64,
    /// Selection changed by keys this frame, so the list scrolls to it.
    scroll: bool,
}

impl Palette {
    fn new(mode: PalMode, now: f64) -> Self {
        Palette { mode, sel: 0, lastq: String::new(), opened: now, scroll: true }
    }
    pub fn is_input(&self) -> bool {
        matches!(self.mode, PalMode::Input { .. })
    }
}

#[derive(Clone)]
struct Snap {
    text: String,
    sel: (usize, usize),
}

pub struct Editor {
    pub path: PathBuf,
    pub title: String,
    pub text: String,
    saved_text: String,
    pub dirty_since: Option<f64>,
    pub palette: Option<Palette>,
    focus_body: bool,
    start_cursor: Option<usize>,
    last_sel: Option<(usize, usize)>,
    boxes: Vec<(Rect, usize)>,
    /// Input that arrived after a key we handled, replayed next frame so keys apply in order.
    pending: Vec<egui::Event>,
    hist: Vec<Snap>,
    fut: Vec<Snap>,
    last_push: f64,
    snap: Snap,
    scrolled_for: Option<usize>,
    had_focus: bool,
    /// A block being dragged by its handle.
    drag: Option<BlockDrag>,
    /// The line whose faint pill and handle show under the pointer.
    hover_line: Option<usize>,
}

struct BlockDrag {
    /// The block's lines.
    block: std::ops::Range<usize>,
    /// Pointer y minus the block's top, in screen space, when the drag began.
    grab: f32,
    /// Which handle started it, so the same widget keeps the drag.
    handle: Id,
    /// Tells one drag's slide animations from the next one's.
    serial: i64,
}

/// What the block menu (the ⋮ handle) can do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlockOp {
    Cut,
    Copy,
    Duplicate,
    Up,
    Down,
    Delete,
}

impl Editor {
    /// Open `item` with the cursor at the end of `line` (or of the last line).
    pub fn new(item: &Item, line: Option<usize>) -> Self {
        let text = item.text.trim_end_matches('\n').to_string();
        let lines: Vec<&str> = text.split('\n').collect();
        let li = line.unwrap_or(lines.len().saturating_sub(1)).min(lines.len().saturating_sub(1));
        let cur: usize = lines[..li].iter().map(|l| l.chars().count() + 1).sum::<usize>() + lines[li].chars().count();
        Editor {
            path: item.path.clone(),
            title: item.name.clone(),
            saved_text: text.clone(),
            snap: Snap { text: text.clone(), sel: (cur, cur) },
            text,
            dirty_since: None,
            palette: None,
            focus_body: true,
            start_cursor: Some(cur),
            last_sel: None,
            boxes: vec![],
            pending: vec![],
            hist: vec![],
            fut: vec![],
            last_push: 0.0,
            scrolled_for: None,
            had_focus: true,
            drag: None,
            hover_line: None,
        }
    }

    fn file_text(&self) -> String {
        if self.text.is_empty() { String::new() } else { format!("{}\n", self.text) }
    }

    pub fn is_saved(&self) -> bool {
        self.text == self.saved_text
    }

    pub fn write_text(&mut self) -> Result<(), String> {
        self.dirty_since = None;
        if !self.is_saved() {
            store::write(&self.path, &self.file_text()).map_err(|e| format!("Couldn't save: {e}"))?;
            self.saved_text = self.text.clone();
        }
        Ok(())
    }

    /// Write the text. Kept returning a path for callers that follow renames.
    pub fn save(&mut self) -> Result<Option<PathBuf>, String> {
        self.write_text()?;
        Ok(None)
    }

    pub fn can_undo(&self) -> bool {
        !self.hist.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.fut.is_empty()
    }

    fn current(&self, ctx: &egui::Context) -> Snap {
        let sel = egui::text_edit::TextEditState::load(ctx, editor_id(&self.path))
            .and_then(|s| s.cursor.char_range())
            .map(|r| (r.secondary.index.0, r.primary.index.0))
            .or(self.last_sel)
            .unwrap_or((0, 0));
        Snap { text: self.text.clone(), sel }
    }

    fn restore(&mut self, ctx: &egui::Context, s: Snap) {
        self.text = s.text.clone();
        let n = self.text.chars().count();
        let (a, b) = (s.sel.0.min(n), s.sel.1.min(n));
        set_cursor(ctx, editor_id(&self.path), a, b);
        ctx.memory_mut(|m| m.request_focus(editor_id(&self.path)));
        self.last_sel = Some((a, b));
        self.snap = Snap { text: self.text.clone(), sel: (a, b) };
        self.palette = None;
        self.dirty_since = Some(ctx.input(|i| i.time));
        self.last_push = 0.0;
        ctx.request_repaint();
    }

    /// Replace the whole text as one undo step, with the cursor at `cursor`.
    fn replace_text(&mut self, ctx: &egui::Context, text: String, cursor: (usize, usize)) {
        if text == self.text {
            return;
        }
        let cur = self.current(ctx);
        self.hist.push(cur);
        if self.hist.len() > 200 {
            self.hist.remove(0);
        }
        self.fut.clear();
        self.restore(ctx, Snap { text, sel: cursor });
    }

    /// Run a block-menu action on the block that line `line` starts.
    pub fn block_op(&mut self, ctx: &egui::Context, op: BlockOp, line: usize, folds: &HashSet<String>) {
        let text = self.text.clone();
        let lines: Vec<&str> = text.split('\n').collect();
        if line >= lines.len() {
            return;
        }
        let blk = blocks::block_at(&lines, line);
        let starts = line_starts(&lines);
        let piece = lines[blk.clone()].join("\n");
        let sel = self.current(ctx).sel;
        match op {
            BlockOp::Copy => {
                ctx.copy_text(format!("{piece}\n"));
            }
            BlockOp::Cut | BlockOp::Delete => {
                if op == BlockOp::Cut {
                    ctx.copy_text(format!("{piece}\n"));
                }
                let mut rest: Vec<&str> = lines[..blk.start].to_vec();
                rest.extend_from_slice(&lines[blk.end..]);
                let t = rest.join("\n");
                // The cursor goes to the start of whatever now fills the gap.
                let at = rest[..blk.start.min(rest.len())].iter().map(|l| l.chars().count() + 1).sum::<usize>().min(t.chars().count());
                self.replace_text(ctx, t, (at, at));
            }
            BlockOp::Duplicate => {
                let mut out: Vec<&str> = lines[..blk.end].to_vec();
                out.extend_from_slice(&lines[blk.clone()]);
                out.extend_from_slice(&lines[blk.end..]);
                let t = out.join("\n");
                let at = starts.get(blk.end).copied().unwrap_or(text.chars().count() + 1) + lines[blk.start].chars().count();
                let at = at.min(t.chars().count());
                self.replace_text(ctx, t, (at, at));
            }
            BlockOp::Up | BlockOp::Down => {
                let (hidden, _) = fold_info(&lines, folds);
                if let Some((t, _, order)) = blocks::nudge(&lines, &blk, &hidden, op == BlockOp::Down) {
                    let a = remap_cursor(&lines, &order, sel.0);
                    let b = remap_cursor(&lines, &order, sel.1);
                    self.replace_text(ctx, t, (a, b));
                }
            }
        }
    }

    /// The note changed on disk (a sync app wrote it). Take the new text in, keeping edits
    /// made here since the last save and the cursor.
    pub fn sync_from_disk(&mut self, ctx: &egui::Context, disk: &str) {
        let disk = disk.trim_end_matches('\n').to_string();
        if disk == self.saved_text {
            return;
        }
        let sel = self.current(ctx).sel;
        let (merged, c1) = blocks::merge(&self.saved_text, &self.text, &disk, sel.1);
        let (_, c0) = blocks::merge(&self.saved_text, &self.text, &disk, sel.0);
        let n = merged.chars().count();
        self.saved_text = disk;
        if merged != self.text {
            self.hist.push(self.current(ctx));
            self.fut.clear();
            self.text = merged;
            let id = editor_id(&self.path);
            set_cursor(ctx, id, c0.min(n), c1.min(n));
            self.last_sel = Some((c0.min(n), c1.min(n)));
            self.snap = Snap { text: self.text.clone(), sel: (c0.min(n), c1.min(n)) };
        }
        // Local edits that the merge kept still need saving.
        self.dirty_since = if self.text != self.saved_text { Some(ctx.input(|i| i.time)) } else { None };
        self.drag = None;
        ctx.request_repaint();
    }

    pub fn undo(&mut self, ctx: &egui::Context) {
        if let Some(s) = self.hist.pop() {
            let cur = self.current(ctx);
            self.fut.push(cur);
            self.restore(ctx, s);
        }
    }

    pub fn redo(&mut self, ctx: &egui::Context) {
        if let Some(s) = self.fut.pop() {
            let cur = self.current(ctx);
            self.hist.push(cur);
            self.restore(ctx, s);
        }
    }
}

/* ---------- text helpers (char indices) ---------- */

/// Char index where each line starts.
fn line_starts(lines: &[&str]) -> Vec<usize> {
    let mut out = Vec::with_capacity(lines.len());
    let mut acc = 0;
    for l in lines {
        out.push(acc);
        acc += l.chars().count() + 1;
    }
    out
}

/// Where char `c` of the old text lands after its lines were reordered (`order[new] = old`).
fn remap_cursor(lines: &[&str], order: &[usize], c: usize) -> usize {
    let starts = line_starts(lines);
    let li = starts.iter().rposition(|&s| s <= c).unwrap_or(0);
    let col = c - starts[li];
    let mut acc = 0;
    for &o in order {
        if o == li {
            return acc + col.min(lines[o].chars().count());
        }
        acc += lines[o].chars().count() + 1;
    }
    c
}

fn byte_at(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}
fn char_at(s: &str, bi: usize) -> usize {
    s[..bi.min(s.len())].chars().count()
}
/// Byte range of the line containing byte `b` (without the newline).
fn line_bounds(s: &str, b: usize) -> (usize, usize) {
    let start = s[..b].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = s[b..].find('\n').map(|i| b + i).unwrap_or(s.len());
    (start, end)
}
fn line_index(s: &str, b: usize) -> usize {
    s[..b.min(s.len())].matches('\n').count()
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cmd {
    H(u8),
    HeadingCycle,
    Bullet,
    Number,
    Task,
    Quote,
    Divider,
    CodeBlock,
    Code,
    Bold,
    Italic,
    Strike,
    Link,
    Indent,
    Outdent,
    Text,
}

/// Apply `cmd` to `text` with selection `sel` (chars). Returns the new selection.
pub fn apply(text: &mut String, sel: (usize, usize), cmd: Cmd, from_menu: bool) -> (usize, usize) {
    let (a, b) = (sel.0.min(sel.1), sel.0.max(sel.1));
    let (ba, bb) = (byte_at(text, a), byte_at(text, b));
    let wrap = |text: &mut String, m: &str| -> (usize, usize) {
        let ml = m.chars().count();
        let inner = &text[ba..bb];
        // Unwrap when the selection is already wrapped (markers inside or just outside).
        if inner.len() >= 2 * m.len() && inner.starts_with(m) && inner.ends_with(m) {
            let s = inner[m.len()..inner.len() - m.len()].to_string();
            text.replace_range(ba..bb, &s);
            return (a, b - 2 * ml);
        }
        if ba >= m.len() && text[..ba].ends_with(m) && text[bb..].starts_with(m) && !(m == "*" && (text[..ba].ends_with("**") || text[bb..].starts_with("**"))) {
            text.replace_range(bb..bb + m.len(), "");
            text.replace_range(ba - m.len()..ba, "");
            return (a - ml, b - ml);
        }
        let s = format!("{m}{inner}{m}");
        text.replace_range(ba..bb, &s);
        (a + ml, b + ml)
    };
    match cmd {
        Cmd::Bold => wrap(text, "**"),
        Cmd::Italic => wrap(text, "*"),
        Cmd::Strike => wrap(text, "~~"),
        Cmd::Code => wrap(text, "`"),
        Cmd::Link => {
            let inner = text[ba..bb].to_string();
            if inner.is_empty() {
                text.replace_range(ba..bb, "[link text](https://)");
                (a + 1, a + 10)
            } else {
                let s = format!("[{inner}](https://)");
                text.replace_range(ba..bb, &s);
                let n = inner.chars().count();
                (a + n + 3, a + n + 11)
            }
        }
        Cmd::Divider => {
            let (ls, le) = line_bounds(text, ba);
            if text[ls..le].trim().is_empty() {
                let has_next = le < text.len();
                text.replace_range(ls..le, "---");
                let after = ls + 3;
                if !has_next {
                    text.insert(after, '\n');
                }
                let c = char_at(text, after) + 1;
                (c, c)
            } else {
                text.insert_str(le, "\n---\n");
                let c = char_at(text, le) + 5;
                (c, c)
            }
        }
        Cmd::CodeBlock => {
            let (ls, le) = line_bounds(text, ba);
            if text[ls..le].trim().is_empty() {
                text.replace_range(ls..le, "```\n\n```");
                let c = char_at(text, ls) + 4;
                (c, c)
            } else {
                text.insert_str(le, "\n```\n\n```");
                let c = char_at(text, le) + 5;
                (c, c)
            }
        }
        Cmd::Indent | Cmd::Outdent => {
            let (ls, _) = line_bounds(text, ba);
            let (_, le) = line_bounds(text, bb);
            let block = text[ls..le].to_string();
            let mut first_delta: i64 = 0;
            let mut total: i64 = 0;
            let out: Vec<String> = block
                .split('\n')
                .enumerate()
                .map(|(i, l)| {
                    let (nl, d) = if cmd == Cmd::Indent {
                        (format!("  {l}"), 2)
                    } else if l.starts_with("  ") {
                        (l[2..].to_string(), -2)
                    } else if l.starts_with('\t') || l.starts_with(' ') {
                        (l[1..].to_string(), -1)
                    } else {
                        (l.to_string(), 0)
                    };
                    if i == 0 {
                        first_delta = d;
                    }
                    total += d;
                    nl
                })
                .collect();
            text.replace_range(ls..le, &out.join("\n"));
            let ls_c = char_at(text, ls) as i64;
            let na = (a as i64 + first_delta).max(ls_c) as usize;
            let nb = (b as i64 + total).max(na as i64) as usize;
            (na, nb)
        }
        _ => {
            // Block formats apply to every line in the selection.
            let (ls, _) = line_bounds(text, ba);
            let (_, le) = line_bounds(text, bb);
            let block = text[ls..le].to_string();
            let lines: Vec<&str> = block.split('\n').collect();
            let cur = md::classify(lines[0]);
            let target = match cmd {
                Cmd::H(n) => Block::Heading(n),
                Cmd::HeadingCycle => match cur.block {
                    Block::Heading(1) => Block::Heading(2),
                    Block::Heading(2) => Block::Heading(3),
                    Block::Heading(_) => Block::Plain,
                    _ => Block::Heading(1),
                },
                Cmd::Bullet => Block::Bullet,
                Cmd::Number => Block::Number,
                Cmd::Task => Block::Task(false),
                Cmd::Quote => Block::Quote,
                _ => Block::Plain,
            };
            let same = |b: Block| match (b, target) {
                (Block::Task(_), Block::Task(_)) => true,
                (x, y) => x == y,
            };
            let toggle_off = !from_menu && cmd != Cmd::HeadingCycle && same(cur.block);
            // Picking a list type from the menu on a line that already has it changes nothing.
            if from_menu && same(cur.block) && matches!(target, Block::Bullet | Block::Number | Block::Task(_) | Block::Quote) {
                return (a, b);
            }
            let mut n = prev_num(text, ls);
            let mut first_delta: i64 = 0;
            let mut total: i64 = 0;
            let out: Vec<String> = lines
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    let c = md::classify(l);
                    let indent = &l[..c.indent];
                    let content = &l[c.marker.end..];
                    let content = if c.block == Block::Rule { "" } else { content };
                    let marker = if toggle_off {
                        String::new()
                    } else {
                        match target {
                            Block::Heading(h) => format!("{} ", "#".repeat(h as usize)),
                            Block::Bullet => "- ".into(),
                            Block::Number => {
                                n += 1;
                                format!("{n}. ")
                            }
                            Block::Task(_) => "- [ ] ".into(),
                            Block::Quote => "> ".into(),
                            _ => String::new(),
                        }
                    };
                    // Headings never keep an indent.
                    let indent = if matches!(target, Block::Heading(_)) && !toggle_off { "" } else { indent };
                    let nl = format!("{indent}{marker}{content}");
                    let d = nl.chars().count() as i64 - l.chars().count() as i64;
                    if i == 0 {
                        first_delta = d;
                    }
                    total += d;
                    nl
                })
                .collect();
            text.replace_range(ls..le, &out.join("\n"));
            let ls_c = char_at(text, ls) as i64;
            let na = (a as i64 + first_delta).max(ls_c) as usize;
            let nb = (b as i64 + total).max(na as i64) as usize;
            if a == b {
                (na, na)
            } else {
                (na, nb)
            }
        }
    }
}

/// Number of the numbered item just above `ls` (skipping blank lines), or 0.
fn prev_num(text: &str, ls: usize) -> u64 {
    for l in text[..ls].split('\n').rev().skip(1) {
        if l.trim().is_empty() {
            continue;
        }
        let c = md::classify(l);
        if c.block == Block::Number {
            let m = l[c.marker.clone()].trim();
            return m.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0);
        }
        return 0;
    }
    0
}

/// Enter on a list line continues the list; on an empty item it ends it.
fn enter_continues(text: &mut String, at: usize) -> Option<usize> {
    let b = byte_at(text, at);
    let (ls, le) = line_bounds(text, b);
    let line = text[ls..le].to_string();
    // Not inside a code block.
    let fences = text[..ls].lines().filter(|l| md::classify(l).block == Block::Fence).count();
    if fences % 2 == 1 {
        return None;
    }
    let c = md::classify(&line);
    let next = match c.block {
        Block::Bullet => format!("{}{} ", &line[..c.indent], &line[c.indent..c.indent + 1]),
        Block::Task(_) => format!("{}- [ ] ", &line[..c.indent]),
        Block::Quote => "> ".to_string(),
        Block::Number => {
            let m = line[c.marker.clone()].trim();
            let digits: String = m.chars().take_while(|c| c.is_ascii_digit()).collect();
            let delim = m.chars().nth(digits.len()).unwrap_or('.');
            let n: u64 = digits.parse().unwrap_or(0);
            format!("{}{}{} ", &line[..c.indent], n + 1, delim)
        }
        _ => return None,
    };
    if b < ls + c.marker.end {
        return None;
    }
    if line[c.content.clone()].trim().is_empty() {
        // Empty item: clear the line.
        text.replace_range(ls..le, "");
        return Some(char_at(text, ls));
    }
    let ins = format!("\n{next}");
    text.insert_str(b, &ins);
    Some(at + ins.chars().count())
}

/* ---------- slash menu ---------- */

pub struct CmdInfo {
    pub cmd: Cmd,
    pub name: &'static str,
    pub desc: &'static str,
    pub icon: Result<Icon, &'static str>,
    pub syntax: &'static str,
    pub keys: &'static [&'static str],
}

pub const CMDS: &[CmdInfo] = &[
    CmdInfo { cmd: Cmd::H(1), name: "Heading 1", desc: "Big section title", icon: Err("H1"), syntax: "#", keys: &["h1", "title", "header", "heading", "big heading", "big header", "headline", "large"] },
    CmdInfo { cmd: Cmd::H(2), name: "Heading 2", desc: "Medium section title", icon: Err("H2"), syntax: "##", keys: &["h2", "subtitle", "subheading", "header", "heading", "section", "medium"] },
    CmdInfo { cmd: Cmd::H(3), name: "Heading 3", desc: "Small section title", icon: Err("H3"), syntax: "###", keys: &["h3", "small heading", "small header", "subsection", "header", "heading", "minor"] },
    CmdInfo { cmd: Cmd::Bullet, name: "Bullet list", desc: "A simple list", icon: Ok(Icon::Bullets), syntax: "-", keys: &["bullet point", "bullets", "bulleted", "dot", "dots", "unordered list", "ul", "list", "point", "dash", "items"] },
    CmdInfo { cmd: Cmd::Number, name: "Numbered list", desc: "Steps in order", icon: Ok(Icon::Numbers), syntax: "1.", keys: &["ordered list", "ol", "numbers", "numbering", "number list", "steps", "enumerate", "sequence", "list", "1."] },
    CmdInfo { cmd: Cmd::Task, name: "Checkbox", desc: "Something to tick off", icon: Ok(Icon::Checkbox), syntax: "- [ ]", keys: &["to-do", "todo", "to do", "task", "tasks", "checklist", "tick", "check", "tickbox"] },
    CmdInfo { cmd: Cmd::Quote, name: "Quote", desc: "Set a passage apart", icon: Ok(Icon::Quote), syntax: ">", keys: &["blockquote", "citation", "cite", "callout", "quotation", "quoted"] },
    CmdInfo { cmd: Cmd::Divider, name: "Divider", desc: "Line across the page", icon: Ok(Icon::Rule), syntax: "---", keys: &["line", "separator", "horizontal rule", "horizontal line", "hr", "rule", "break", "split", "section break"] },
    CmdInfo { cmd: Cmd::CodeBlock, name: "Code block", desc: "Monospace block", icon: Ok(Icon::Code), syntax: "```", keys: &["snippet", "pre", "preformatted", "fence", "program", "code", "terminal", "script"] },
    CmdInfo { cmd: Cmd::Code, name: "Inline code", desc: "Code inside a sentence", icon: Ok(Icon::Code), syntax: "`code`", keys: &["code", "mono", "monospace", "backtick", "command"] },
    CmdInfo { cmd: Cmd::Bold, name: "Bold", desc: "Strong text", icon: Err("B"), syntax: "**text**", keys: &["strong", "fat", "thick", "heavy", "emphasis", "important"] },
    CmdInfo { cmd: Cmd::Italic, name: "Italic", desc: "Slanted text", icon: Err("I"), syntax: "*text*", keys: &["emphasis", "emphasize", "slanted", "cursive", "oblique", "em", "lean"] },
    CmdInfo { cmd: Cmd::Strike, name: "Strikethrough", desc: "Crossed-out text", icon: Err("S"), syntax: "~~text~~", keys: &["cross out", "crossed out", "strike", "line through", "done", "cancel", "scratch"] },
    CmdInfo { cmd: Cmd::Link, name: "Link", desc: "Web address with a label", icon: Ok(Icon::Link), syntax: "[text](url)", keys: &["url", "hyperlink", "web", "href", "address", "website", "www"] },
    CmdInfo { cmd: Cmd::Indent, name: "Indent", desc: "Nest under the line above", icon: Ok(Icon::Indent), syntax: "Tab", keys: &["nest", "sub item", "subitem", "child", "tab", "move right", "deeper", "sublist"] },
    CmdInfo { cmd: Cmd::Outdent, name: "Outdent", desc: "Move one level out", icon: Ok(Icon::Outdent), syntax: "⇧Tab", keys: &["unnest", "move left", "shallower", "dedent", "unindent", "out"] },
    CmdInfo { cmd: Cmd::Text, name: "Plain text", desc: "Remove line formatting", icon: Ok(Icon::Text), syntax: "", keys: &["paragraph", "normal", "body", "clear formatting", "remove formatting", "plain", "reset", "none"] },
];

fn score(c: &CmdInfo, q: &str) -> f32 {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return 1.0;
    }
    let name = c.name.to_lowercase();
    let terms: Vec<&str> = std::iter::once(name.as_str()).chain(c.keys.iter().copied()).collect();
    let qw: Vec<&str> = q.split_whitespace().collect();
    let mut best: f32 = 0.0;
    for (ti, t) in terms.iter().enumerate() {
        let w = if ti == 0 { 1.0 } else { 0.85 };
        if *t == q {
            best = best.max(10.0 * w)
        } else if t.starts_with(&q) {
            best = best.max(8.0 * w - t.len() as f32 * 0.01)
        } else if qw.iter().all(|x| t.split([' ', '-']).any(|tw| tw.starts_with(x))) {
            best = best.max(6.0 * w)
        } else if q.len() >= 3 && t.contains(&q) {
            best = best.max(3.0 * w)
        }
    }
    if best == 0.0 && q.len() >= 3 {
        // Forgiving subsequence match on the name, for typos like "bulet".
        let qc: Vec<char> = q.chars().collect();
        let mut j = 0;
        for ch in name.chars() {
            if j < qc.len() && ch == qc[j] {
                j += 1
            }
        }
        if j == qc.len() {
            best = 1.0
        }
    }
    best
}

fn matches(q: &str) -> Vec<&'static CmdInfo> {
    let mut v: Vec<(f32, usize, &CmdInfo)> = CMDS.iter().enumerate().map(|(i, c)| (score(c, q), i, c)).filter(|x| x.0 > 0.0).collect();
    v.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
    v.into_iter().map(|x| x.2).collect()
}

/* ---------- layout ---------- */

enum DecoKind {
    Dot,
    Box(bool),
    Bar,
    Rule,
    Code { first: bool, last: bool },
}

struct Deco {
    line: usize,
    /// Char index where the decoration sits, and where its line ends.
    at: usize,
    end: usize,
    kind: DecoKind,
}

fn fmt(font: FontId, color: Color32) -> TextFormat {
    TextFormat { font_id: font, color, line_height: Some(LH), ..Default::default() }
}

fn heading_font(level: u8) -> FontId {
    match level {
        1 => xbold(22.0),
        2 => xbold(19.0),
        3 => bold(17.0),
        _ => bold(16.0),
    }
}

/// Note starts with a `# ` heading, which then stands in for the title.
pub fn starts_with_h1(text: &str) -> bool {
    text.lines().find(|l| !l.trim().is_empty()).map(|l| l.starts_with("# ") || l.trim() == "#").unwrap_or(false)
}

/// First line after the section that heading `i` starts.
pub fn section_end(lines: &[&str], i: usize) -> usize {
    let lv = match md::classify(lines[i]).block {
        Block::Heading(l) => l,
        _ => return i + 1,
    };
    let mut fence = false;
    for (j, l) in lines.iter().enumerate().skip(i + 1) {
        let c = md::classify(l);
        if c.block == Block::Fence {
            fence = !fence;
            continue;
        }
        if !fence {
            if let Block::Heading(h) = c.block {
                if h <= lv {
                    return j;
                }
            }
        }
    }
    lines.len()
}

/// Lines hidden under folded headings, and the visible foldable headings with their state.
fn fold_info(lines: &[&str], folds: &HashSet<String>) -> (HashSet<usize>, Vec<(usize, bool)>) {
    let mut hidden = HashSet::new();
    let mut heads = Vec::new();
    let mut fence = false;
    for i in 0..lines.len() {
        let c = md::classify(lines[i]);
        if c.block == Block::Fence {
            fence = !fence;
            continue;
        }
        if fence || hidden.contains(&i) || !matches!(c.block, Block::Heading(_)) {
            continue;
        }
        let end = section_end(lines, i);
        if !lines[i + 1..end].iter().any(|l| !l.trim().is_empty()) {
            continue;
        }
        let closed = folds.contains(lines[i].trim());
        heads.push((i, closed));
        if closed {
            hidden.extend(i + 1..end);
        }
    }
    (hidden, heads)
}

type Measure<'a> = &'a dyn Fn(&str, &FontId) -> f32;

/// Format that stretches or squeezes `text` to exactly `target` pixels.
fn spaced(text: &str, font: FontId, color: Color32, target: f32, m: Measure) -> TextFormat {
    let n = text.chars().count().max(1) as f32;
    let mut f = fmt(font.clone(), color);
    f.extra_letter_spacing = (target - m(text, &font)) / n;
    f
}

/// Style the whole note. `active` is the line holding the cursor: it shows as plain text.
fn build(text: &str, active: Option<usize>, hidden: &HashSet<usize>, wrap: f32, m: Measure) -> (LayoutJob, Vec<Deco>) {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let mut decos = Vec::new();
    let hidden_fmt = fmt(sans(1.0), Color32::TRANSPARENT);
    let gone = TextFormat { font_id: sans(1.0), color: Color32::TRANSPARENT, line_height: Some(0.0), ..Default::default() };
    let mut chars = 0usize;
    let mut in_code = false;
    let all: Vec<&str> = text.split('\n').collect();
    let n = all.len();
    for (li, line) in all.iter().enumerate() {
        let is_active = active == Some(li);
        let lc = line.chars().count();
        let c = md::classify(line);
        let is_fence = c.block == Block::Fence;
        let code_line = is_fence || in_code;
        let opening = is_fence && !in_code;
        let closing = is_fence && in_code;
        if opening {
            in_code = true;
        }
        let nl_fmt;
        if hidden.contains(&li) {
            job.append(line, 0.0, gone.clone());
            nl_fmt = gone.clone();
        } else if code_line {
            let color = if is_active { WHITE } else if is_fence { SYNTAX } else { TEXT2 };
            let f = fmt(mono(13.0), color);
            job.append(line, 0.0, f.clone());
            decos.push(Deco { line: li, at: chars, end: chars + lc, kind: DecoKind::Code { first: opening, last: closing || li == n - 1 } });
            nl_fmt = f;
        } else if is_active {
            let base = match c.block {
                Block::Heading(l) => heading_font(l),
                _ => sans(16.0),
            };
            let f = fmt(base, WHITE);
            job.append(line, 0.0, f.clone());
            nl_fmt = f;
        } else {
            let (base, color) = match c.block {
                Block::Heading(l) => (heading_font(l), WHITE),
                Block::Bullet | Block::Number | Block::Task(_) => (sans(16.0), WHITE),
                Block::Quote => (italic(16.0), TEXT3),
                _ => (sans(16.0), TEXT2),
            };
            let done = matches!(c.block, Block::Task(true));
            if c.indent > 0 {
                let ind = &line[..c.indent];
                if matches!(c.block, Block::Bullet | Block::Number | Block::Task(_)) {
                    // Each nesting level steps in 22px, like the reader.
                    job.append(ind, 0.0, spaced(ind, sans(16.0), color, c.level(line) as f32 * 22.0, m));
                } else {
                    job.append(ind, 0.0, fmt(sans(16.0), color));
                }
            }
            let marker = &line[c.marker.clone()];
            let at = chars + line[..c.marker.start].chars().count();
            match c.block {
                Block::Heading(_) => job.append(marker, 0.0, hidden_fmt.clone()),
                Block::Number => {
                    let t = marker.trim_end();
                    job.append(t, 0.0, fmt(sans(15.0), MUTED));
                    let sp = &marker[t.len()..];
                    if !sp.is_empty() {
                        job.append(sp, 0.0, spaced(sp, sans(16.0), MUTED, 8.0, m));
                    }
                }
                Block::Bullet => {
                    job.append(marker, 0.0, spaced(marker, sans(16.0), Color32::TRANSPARENT, 15.0, m));
                    decos.push(Deco { line: li, at, end: chars + lc, kind: DecoKind::Dot });
                }
                Block::Task(d) => {
                    job.append(marker, 0.0, spaced(marker, sans(16.0), Color32::TRANSPARENT, 26.0, m));
                    decos.push(Deco { line: li, at, end: chars + lc, kind: DecoKind::Box(d) });
                }
                Block::Quote => {
                    job.append(marker, 0.0, spaced(marker, sans(16.0), Color32::TRANSPARENT, 17.0, m));
                    decos.push(Deco { line: li, at, end: chars + lc, kind: DecoKind::Bar });
                }
                Block::Rule => decos.push(Deco { line: li, at, end: chars + lc, kind: DecoKind::Rule }),
                _ => {
                    if !marker.is_empty() {
                        job.append(marker, 0.0, fmt(base.clone(), SYNTAX))
                    }
                }
            }
            if c.block == Block::Rule {
                job.append(&line[c.indent..], 0.0, fmt(base.clone(), Color32::TRANSPARENT));
            } else {
                for sp in md::inline(line, c.content.clone()) {
                    let t = &line[sp.range.clone()];
                    if sp.style.marker {
                        job.append(t, 0.0, hidden_fmt.clone());
                        continue;
                    }
                    let mut f = format_for(sp.style, &base, color, LH);
                    if done {
                        f.color = MUTED;
                        f.strikethrough = Stroke::new(1.5, MUTED);
                    }
                    job.append(t, 0.0, f);
                }
            }
            nl_fmt = fmt(base, color);
        }
        if closing {
            in_code = false;
        }
        chars += lc;
        if li + 1 < n {
            job.append("\n", 0.0, nl_fmt);
            chars += 1;
        }
    }
    if job.text.is_empty() {
        job.append("", 0.0, fmt(sans(16.0), TEXT2));
    }
    (job, decos)
}

fn editor_id(path: &PathBuf) -> Id {
    Id::new(("editor", path))
}

fn set_cursor(ctx: &egui::Context, id: Id, a: usize, b: usize) {
    let mut st = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
    st.store(ctx, id);
}

/// Inline styles at the cursor, for the toolbar's highlighted buttons.
fn styles_at(text: &str, at: usize) -> (Block, bool, bool, bool) {
    let b = byte_at(text, at);
    let (ls, le) = line_bounds(text, b);
    let line = &text[ls..le];
    let c = md::classify(line);
    let off = b - ls;
    let mut st = (c.block, false, false, false);
    for sp in md::inline(line, c.content.clone()) {
        if sp.range.start < off && off <= sp.range.end && !sp.style.marker {
            st.1 = sp.style.bold;
            st.2 = sp.style.italic;
            st.3 = sp.style.strike;
        }
    }
    st
}

/// Space kept free at the right of the text for the block handle.
const HANDLE_ROOM: f32 = 22.0;
/// Selected text sits on a soft white highlight with rounded corners.
const SELECTION: Color32 = Color32::from_rgba_premultiplied(70, 70, 70, 70);

/// Where a line's text starts after its indent and marker, so wrapped rows line up with it.
fn hang_points(lines: &[&str], hidden: &HashSet<usize>) -> Vec<Option<usize>> {
    let mut out = Vec::with_capacity(lines.len());
    let mut fence = false;
    for (i, l) in lines.iter().enumerate() {
        let c = md::classify(l);
        let is_fence = c.block == Block::Fence;
        let code = fence || is_fence;
        if is_fence {
            fence = !fence;
        }
        out.push(if code || hidden.contains(&i) {
            None
        } else {
            match c.block {
                Block::Bullet | Block::Number | Block::Task(_) | Block::Quote => Some(c.content.start),
                Block::Plain if c.indent > 0 => Some(c.indent),
                _ => None,
            }
        });
    }
    out
}

/// The active-line pill: a near-black card with a fine light outline.
fn pill_shapes(r: Rect, a: f32) -> [Shape; 2] {
    [
        Shape::rect_filled(r, 12, Color32::from_gray(0x11).gamma_multiply(a)),
        Shape::rect_stroke(r, 12, Stroke::new(1.0, Color32::from_gray(0x2a).gamma_multiply(a)), StrokeKind::Inside),
    ]
}

/// The block handle's spot: inside the pill's right end, centred on the line's first row.
fn handle_rect(pill_right: f32, line_top: f32) -> Rect {
    Rect::from_center_size(pos2(pill_right - 4.0 - 13.0, line_top + LH / 2.0), vec2(26.0, 26.0))
}

fn paint_handle(p: &egui::Painter, r: Rect, a: f32, hover: f32) {
    if a <= 0.0 {
        return;
    }
    let bg = widgets::lerp_color(Color32::from_gray(0x1f), Color32::from_gray(0x2e), hover);
    p.rect_filled(r, 9, bg.gamma_multiply(a));
    let fg = widgets::lerp_color(Color32::from_gray(0xb5), WHITE, hover);
    icons::paint(p, Rect::from_center_size(r.center(), vec2(15.0, 15.0)), Icon::Grip, fg.gamma_multiply(a));
}

/// Key hint chip drawn from its left edge; returns its width.
fn chip(p: &egui::Painter, x: f32, cy: f32, text: &str) -> f32 {
    let g = p.layout_no_wrap(text.into(), mono(11.0), TEXT2);
    let w = g.size().x + 10.0;
    let r = Rect::from_min_size(pos2(x, cy - 9.0), vec2(w, 18.0));
    p.rect_stroke(r, 6, Stroke::new(LINE, BORDER), StrokeKind::Inside);
    p.galley(pos2(r.min.x + 5.0, r.center().y - g.size().y / 2.0), g, TEXT2);
    w
}

enum Tool {
    Btn(Result<Icon, &'static str>, Cmd, bool, &'static str, String),
    Sep,
    Slash,
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let Some(path) = app.editor.as_ref().map(|e| e.path.clone()) else { return };
    let folds: HashSet<String> = app.folds.get(&path).cloned().unwrap_or_default();
    let mut fold_toggle: Option<String> = None;
    let e = app.editor.as_mut().unwrap();
    let id = editor_id(&e.path);
    let rect = ui.max_rect();

    // First frame: focus the text with the cursor where the note was opened.
    if let Some(c) = e.start_cursor.take() {
        set_cursor(&ctx, id, c, c);
        ctx.memory_mut(|m| m.request_focus(id));
        e.last_sel = Some((c, c));
        e.snap.sel = (c, c);
    }
    let focused = ctx.memory(|m| m.has_focus(id));
    let mut focused = focused;
    // egui drops focus on Escape before we see the key; keep it while the slash menu uses Esc.
    let esc = ctx.input(|i| i.events.iter().any(|ev| matches!(ev, egui::Event::Key { key: Key::Escape, pressed: true, .. })));
    if !focused && esc && e.had_focus && ctx.memory(|m| m.focused().is_none()) {
        ctx.memory_mut(|m| m.request_focus(id));
        focused = true;
    }
    let state = egui::text_edit::TextEditState::load(&ctx, id);
    let mut sel = state.as_ref().and_then(|s| s.cursor.char_range()).map(|r| (r.secondary.index.0, r.primary.index.0));
    if focused {
        if let Some(s) = sel {
            e.last_sel = Some(s);
        }
    }
    let mut cursor = sel.map(|s| s.1).or(e.last_sel.map(|s| s.1)).unwrap_or(0);
    let mut new_sel: Option<(usize, usize)> = None;
    let mut changed = false;
    let mut force_hist = false;
    let mut refocus = false;

    // A click on a checkbox ticks it without moving the cursor.
    let mut tick: Option<usize> = None;
    if let Some(p) = ctx.input(|i| if i.pointer.primary_pressed() { i.pointer.interact_pos() } else { None }) {
        if let Some((_, line)) = e.boxes.iter().find(|(r, _)| r.expand(3.0).contains(p)) {
            tick = Some(*line);
        }
    }

    // The palette with its own search field takes the arrow keys, Enter and Esc first.
    let mut input_choice: Option<(Cmd, (usize, usize))> = None;
    let mut input_close: Option<(usize, usize)> = None;
    if let Some(p) = &mut e.palette {
        if let PalMode::Input { query, at, .. } = &p.mode {
            let list = matches(query);
            let (up, down, tab, stab, enter, esc) = ctx.input_mut(|i| {
                let stab = i.consume_key(Modifiers::SHIFT, Key::Tab);
                (
                    i.consume_key(Modifiers::NONE, Key::ArrowUp),
                    i.consume_key(Modifiers::NONE, Key::ArrowDown),
                    i.consume_key(Modifiers::NONE, Key::Tab),
                    stab,
                    i.consume_key(Modifiers::NONE, Key::Enter),
                    i.consume_key(Modifiers::NONE, Key::Escape),
                )
            });
            if !list.is_empty() {
                if down || tab {
                    p.sel = (p.sel + 1) % list.len();
                    p.scroll = true;
                }
                if up || stab {
                    p.sel = (p.sel + list.len() - 1) % list.len();
                    p.scroll = true;
                }
                if enter {
                    input_choice = Some((list[p.sel.min(list.len() - 1)].cmd, *at));
                }
            }
            if esc {
                input_close = Some(*at);
            }
        }
    }
    if let Some((c, at)) = input_choice {
        e.palette = None;
        new_sel = Some(apply(&mut e.text, at, c, true));
        changed = true;
        force_hist = true;
        refocus = true;
    }
    if let Some(at) = input_close {
        e.palette = None;
        new_sel = Some(at);
        refocus = true;
    }

    // Keys the editor handles itself, before the text field sees them.
    let mut deferred: Vec<egui::Event> = Vec::new();
    if focused && !e.pending.is_empty() {
        let mut replay = std::mem::take(&mut e.pending);
        ctx.input_mut(|i| {
            replay.extend(i.events.drain(..));
            i.events = replay;
        });
    } else {
        e.pending.clear();
    }
    if focused {
        // Text typed in the same frame before a key we handle ourselves goes in first, so
        // fast typing ("item" then Enter) keeps its order.
        let flushed: String = ctx.input_mut(|i| {
            let first_key = i.events.iter().position(|ev| match ev {
                egui::Event::Key { key, pressed: true, modifiers, .. } => {
                    matches!(key, Key::Enter | Key::Tab | Key::ArrowUp | Key::ArrowDown | Key::Escape | Key::Backspace) || modifiers.command
                }
                _ => false,
            });
            let Some(k) = first_key else { return String::new() };
            // Handle one such key per frame; replay what follows it next frame.
            let rest = i.events.split_off(k + 1);
            if rest.iter().any(|ev| matches!(ev, egui::Event::Key { .. } | egui::Event::Text(_) | egui::Event::Paste(_))) {
                deferred = rest;
            } else {
                i.events.extend(rest);
            }
            let mut t = String::new();
            let mut idx = 0;
            i.events.retain(|ev| {
                let keep = match ev {
                    egui::Event::Text(s) if idx < k => {
                        t.push_str(s);
                        false
                    }
                    _ => true,
                };
                idx += 1;
                keep
            });
            t
        });
        if !flushed.is_empty() {
            let (a, b) = sel.unwrap_or((cursor, cursor));
            let (a, b) = (a.min(b), a.max(b));
            let (ba, bb) = (byte_at(&e.text, a), byte_at(&e.text, b));
            e.text.replace_range(ba..bb, &flushed);
            let c = a + flushed.chars().count();
            // A slash typed in this burst can open the menu.
            if e.palette.is_none() {
                if let Some(rel) = flushed.rfind('/') {
                    let slash = a + flushed[..rel].chars().count();
                    let before = if slash == 0 { None } else { e.text.chars().nth(slash - 1) };
                    let tail = &flushed[rel + 1..];
                    if matches!(before, None | Some(' ') | Some('\n') | Some('\t')) && !tail.contains(' ') {
                        e.palette = Some(Palette::new(PalMode::Slash(slash), now));
                    }
                }
            }
            cursor = c;
            sel = Some((c, c));
            new_sel = Some((c, c));
            changed = true;
        }
        let s = sel.unwrap_or((cursor, cursor));
        let cmd = Modifiers::COMMAND;
        let slash_pal = e.palette.as_ref().and_then(|p| if let PalMode::Slash(sl) = p.mode { Some(sl) } else { None });
        if let Some(slash) = slash_pal {
            let p = e.palette.as_mut().unwrap();
            let q_end = cursor;
            let q: String = e.text.chars().skip(slash + 1).take(q_end.saturating_sub(slash + 1)).collect();
            let list = matches(&q);
            let (up, down, tab, stab, enter, esc) = ctx.input_mut(|i| {
                let stab = i.consume_key(Modifiers::SHIFT, Key::Tab);
                (
                    i.consume_key(Modifiers::NONE, Key::ArrowUp),
                    i.consume_key(Modifiers::NONE, Key::ArrowDown),
                    i.consume_key(Modifiers::NONE, Key::Tab),
                    stab,
                    i.consume_key(Modifiers::NONE, Key::Enter),
                    i.consume_key(Modifiers::NONE, Key::Escape),
                )
            });
            if !list.is_empty() {
                if down || tab {
                    p.sel = (p.sel + 1) % list.len();
                    p.scroll = true;
                }
                if up || stab {
                    p.sel = (p.sel + list.len() - 1) % list.len();
                    p.scroll = true;
                }
            }
            if esc {
                // Keep the typed `/` as plain text.
                e.palette = None;
            } else if enter {
                if let Some(c) = list.get(p.sel.min(list.len().saturating_sub(1))) {
                    let (b0, b1) = (byte_at(&e.text, slash), byte_at(&e.text, q_end));
                    // Record the literal text first so undo brings it back.
                    e.hist.push(Snap { text: e.text.clone(), sel: (q_end, q_end) });
                    e.fut.clear();
                    e.text.replace_range(b0..b1, "");
                    e.snap = Snap { text: e.text.clone(), sel: (slash, slash) };
                    new_sel = Some(apply(&mut e.text, (slash, slash), c.cmd, true));
                    changed = true;
                    force_hist = true;
                }
                e.palette = None;
            }
        } else {
            let k = |m: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(m, key));
            let mut run = None;
            if k(cmd, Key::B) {
                run = Some(Cmd::Bold)
            } else if k(cmd, Key::I) {
                run = Some(Cmd::Italic)
            } else if k(cmd, Key::K) {
                run = Some(Cmd::Link)
            } else if k(cmd, Key::Backtick) {
                run = Some(Cmd::Code)
            } else if k(cmd | Modifiers::SHIFT, Key::X) {
                run = Some(Cmd::Strike)
            } else if k(cmd | Modifiers::SHIFT, Key::Period) || k(cmd | Modifiers::SHIFT, Key::Colon) {
                run = Some(Cmd::Quote)
            } else if k(cmd | Modifiers::ALT, Key::Num1) {
                run = Some(Cmd::H(1))
            } else if k(cmd | Modifiers::ALT, Key::Num2) {
                run = Some(Cmd::H(2))
            } else if k(cmd | Modifiers::ALT, Key::Num3) {
                run = Some(Cmd::H(3))
            } else if k(cmd | Modifiers::SHIFT, Key::Num8) {
                run = Some(Cmd::Bullet)
            } else if k(cmd | Modifiers::SHIFT, Key::Num7) {
                run = Some(Cmd::Number)
            } else if k(cmd | Modifiers::SHIFT, Key::Num9) {
                run = Some(Cmd::Task)
            } else if k(Modifiers::SHIFT, Key::Tab) {
                run = Some(Cmd::Outdent)
            } else if k(Modifiers::NONE, Key::Tab) {
                run = Some(Cmd::Indent)
            }
            if let Some(c) = run {
                new_sel = Some(apply(&mut e.text, s, c, false));
                changed = true;
                force_hist = true;
            } else if k(cmd | Modifiers::SHIFT, Key::Z) || k(cmd, Key::Y) {
                e.redo(&ctx);
                new_sel = Some(e.snap.sel);
            } else if k(cmd, Key::Z) {
                e.undo(&ctx);
                new_sel = Some(e.snap.sel);
            } else if k(Modifiers::ALT, Key::ArrowUp) || k(Modifiers::ALT, Key::ArrowDown) {
                let down = ctx.input(|i| i.key_down(Key::ArrowDown));
                let li = line_index(&e.text, byte_at(&e.text, s.1));
                e.block_op(&ctx, if down { BlockOp::Down } else { BlockOp::Up }, li, &folds);
                new_sel = Some(e.snap.sel);
            } else if k(cmd, Key::Slash) {
                e.palette = Some(Palette::new(PalMode::Input { query: String::new(), at: s, focus: true }, now));
            } else if s.0 == s.1 && ctx.input(|i| i.key_pressed(Key::Enter) && i.modifiers.is_none()) {
                let mut t = e.text.clone();
                if let Some(c) = enter_continues(&mut t, s.1) {
                    ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                    e.text = t;
                    new_sel = Some((c, c));
                    changed = true;
                }
                force_hist = true;
            } else if s.0 == s.1 && ctx.input(|i| i.key_pressed(Key::Backspace) && i.modifiers.is_none()) {
                // Backspace right after a list, quote or heading marker removes the marker.
                let b = byte_at(&e.text, s.1);
                let (ls, le) = line_bounds(&e.text, b);
                let c = md::classify(&e.text[ls..le]);
                if matches!(c.block, Block::Bullet | Block::Number | Block::Task(_) | Block::Quote | Block::Heading(_))
                    && c.marker.end > c.marker.start
                    && b - ls == c.marker.end
                {
                    ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Backspace));
                    e.text.replace_range(ls + c.marker.start..ls + c.marker.end, "");
                    let nc = char_at(&e.text, ls + c.marker.start);
                    new_sel = Some((nc, nc));
                    changed = true;
                    force_hist = true;
                }
            }
        }
    }
    if let Some((a, b)) = new_sel {
        set_cursor(&ctx, id, a, b);
    }
    if refocus {
        ctx.memory_mut(|m| m.request_focus(id));
    }
    let cur_now = new_sel.map(|s| s.1).unwrap_or(cursor).min(e.text.chars().count());
    let active = Some(line_index(&e.text, byte_at(&e.text, cur_now)));

    let mut toolbar_cmd: Option<Cmd> = None;
    let mut open_input = false;
    let mut pal_click: Option<Cmd> = None;
    let mut cursor_screen: Option<Rect> = None;
    let mut pal_anchor: Option<Rect> = None;
    let mut boxes = Vec::new();
    let mut post_cursor = cur_now;
    let _ = &mut post_cursor;
    let title = if starts_with_h1(&e.text) { None } else { Some(e.title.clone()) };
    let mut drop_at: Option<(std::ops::Range<usize>, usize, usize)> = None;
    let mut menu_req: Option<(Rect, usize)> = None;
    let mut handles: Vec<(Rect, usize, Id, f32)> = Vec::new();
    let mut lift_bg: Vec<Shape> = Vec::new();
    let mut sel_shapes: Vec<Shape> = Vec::new();
    if e.drag.is_some() && ctx.input(|i| i.key_pressed(Key::Escape)) {
        e.drag = None;
    }

    egui::ScrollArea::vertical().id_salt(("editor-scroll", &e.path)).auto_shrink([false, false]).show(ui, |ui| {
        let avail = ui.available_width();
        let (left, w) = reader::column(avail);
        let x0 = ui.max_rect().min.x + left;
        ui.add_space(8.0);
        if let Some(t) = &title {
            let mut tj = widgets::simple_job(t, xbold(30.0), WHITE, w, 0);
            tj.sections[0].format.line_height = Some(36.0);
            let g = widgets::layout(ui, tj);
            let (r, _) = ui.allocate_exact_size(vec2(avail, g.size().y), Sense::hover());
            ui.painter().galley(pos2(x0, r.min.y), g, WHITE);
            ui.add_space(16.0);
        }

        // Body
        let top = ui.cursor().min.y;
        let bg_idx = ui.painter().add(Shape::Noop);
        let mut decos: Vec<Deco> = Vec::new();
        let dragging = e.drag.is_some();
        // Text wraps a little short of the column so the block handle has room.
        let tw = (w - HANDLE_ROOM).max(80.0);
        let lay = |ui: &Ui, t: &str, wrap: f32, decos: &mut Vec<Deco>| -> Arc<Galley> {
            let lines: Vec<&str> = t.split('\n').collect();
            let (hidden, _) = fold_info(&lines, &folds);
            let m = |s: &str, f: &FontId| ui.fonts_mut(|fo| fo.layout_no_wrap(s.to_string(), f.clone(), WHITE).size().x);
            let (job, d) = build(t, active, &hidden, wrap, &m);
            *decos = d;
            hang::layout(ui, job, &hang_points(&lines, &hidden))
        };
        let mut te_out = None;
        let (g, gp) = if !dragging {
            let out = ui
                .scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(x0, top), vec2(w, f32::INFINITY))), |ui| {
                    // The selection is painted below as rounded highlights instead.
                    ui.visuals_mut().selection.bg_fill = Color32::TRANSPARENT;
                    let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| lay(ui, buf.as_str(), wrap, &mut decos);
                    egui::TextEdit::multiline(&mut e.text)
                        .id(id)
                        .font(sans(16.0))
                        .text_color(TEXT2)
                        .frame(egui::Frame::NONE)
                        .margin(egui::Margin::ZERO)
                        .desired_width(tw)
                        .desired_rows(1)
                        .lock_focus(true)
                        .hint_text(egui::RichText::new("Start writing…  type / for formatting").font(sans(16.0)).color(Color32::from_gray(0x55)))
                        .layouter(&mut layouter)
                        .show(ui)
                })
                .inner;
            if out.response.response.changed() {
                changed = true;
            }
            post_cursor = out.cursor_range.map(|r| r.primary.index.0).unwrap_or(cur_now).min(e.text.chars().count());
            let r = (out.galley.clone(), out.galley_pos);
            te_out = Some(out);
            r
        } else {
            // While a block is dragged the text is painted piece by piece, so it isn't edited.
            let g = lay(ui, &e.text, tw, &mut decos);
            let (r, _) = ui.allocate_exact_size(vec2(avail, g.size().y.max(LH)), Sense::hover());
            (g, pos2(x0, r.min.y))
        };
        let lines: Vec<&str> = e.text.split('\n').collect();
        let starts = line_starts(&lines);
        let acc = starts.last().map(|s| s + lines.last().map(|l| l.chars().count()).unwrap_or(0) + 1).unwrap_or(0);
        let cur_line = line_index(&e.text, byte_at(&e.text, post_cursor));
        let (hidden_now, heads) = fold_info(&lines, &folds);

        // Each line's rows, and its top and bottom relative to the galley.
        let mut line_rows: Vec<std::ops::Range<usize>> = vec![0..0; lines.len()];
        {
            let mut li = 0;
            let mut first = 0;
            for (ri, row) in g.rows.iter().enumerate() {
                if row.ends_with_newline || ri + 1 == g.rows.len() {
                    if li < line_rows.len() {
                        line_rows[li] = first..ri + 1;
                    }
                    li += 1;
                    first = ri + 1;
                }
            }
        }
        let ltop = |li: usize| g.rows.get(line_rows[li].start).map(|r| r.pos.y).unwrap_or(0.0);
        let lbot = |li: usize| line_rows[li].clone().last().and_then(|r| g.rows.get(r)).map(|r| r.pos.y + r.size.y).unwrap_or(0.0);
        let line_of = |ci: usize| starts.partition_point(|&s| s <= ci).saturating_sub(1).min(lines.len().saturating_sub(1));

        // Dragging: where every other piece slides to, and where the block would land.
        let mut off = vec![0.0f32; lines.len()];
        let mut lifted: Option<(std::ops::Range<usize>, f32)> = None;
        if let Some(d) = &e.drag {
            let blk = d.block.start.min(lines.len())..d.block.end.min(lines.len());
            let ptr = ctx.input(|i| i.pointer.latest_pos()).map(|p| p.y).unwrap_or(gp.y + ltop(blk.start) + d.grab);
            let (btop, bbot) = (ltop(blk.start), lbot(blk.end.saturating_sub(1).max(blk.start)));
            let hgt = bbot - btop;
            let want = ptr - d.grab - gp.y;
            let centre = want + hgt / 2.0;
            let us = blocks::units(&lines, &blk, &hidden_now);
            let mut gap = 0;
            for (j, u) in us.iter().enumerate() {
                let (ut, ub) = (ltop(u.start), lbot(u.end - 1));
                let ct = if ut >= bbot { ut - hgt } else { ut };
                if ct + (ub - ut) / 2.0 < centre {
                    gap = j + 1;
                }
            }
            for (j, u) in us.iter().enumerate() {
                let ut = ltop(u.start);
                let ct = if ut >= bbot { ut - hgt } else { ut };
                let target = ct + if j >= gap { hgt } else { 0.0 } - ut;
                let o = ctx.animate_value_with_time(id.with(("unit", d.serial, u.start)), target, 0.16);
                for li in u.clone() {
                    off[li] = o;
                }
            }
            for li in blk.clone() {
                off[li] = want - btop;
            }
            lifted = Some((blk.clone(), want - btop));
            // Scroll when the block is held near the top or bottom edge.
            let clip = ui.clip_rect();
            if ptr < clip.min.y + 40.0 {
                ui.scroll_with_delta(vec2(0.0, 10.0));
            } else if ptr > clip.max.y - 110.0 {
                ui.scroll_with_delta(vec2(0.0, -10.0));
            }
            ctx.request_repaint();
            if !ctx.input(|i| i.pointer.primary_down()) {
                drop_at = Some((blk, gap, blocks::home_gap(&d.block, &us)));
            }
        }
        let row_rect = |ci: usize| -> Rect { g.pos_from_cursor(CCursor::new(ci)).translate(gp.to_vec2() + vec2(0.0, off[line_of(ci)])) };
        let lift_layer = egui::LayerId::new(egui::Order::Middle, id.with("lifted"));
        let lift_painter = ctx.layer_painter(lift_layer).with_clip_rect(ui.clip_rect());
        let base_painter = ui.painter().clone();
        let pnt = |li: usize| -> &egui::Painter {
            match &lifted {
                Some((b, _)) if b.contains(&li) => &lift_painter,
                _ => &base_painter,
            }
        };

        // Background: code blocks, then the active-line pill.
        let mut bg = Vec::new();
        let mut block: Option<f32> = None;
        for d in &decos {
            if let DecoKind::Code { first, last } = d.kind {
                let r0 = row_rect(d.at);
                if first || block.is_none() {
                    block = Some(r0.min.y);
                }
                if last {
                    let top = block.take().unwrap_or(r0.min.y);
                    let r1 = row_rect(d.end);
                    let r = Rect::from_min_max(pos2(gp.x - 10.0, top), pos2(gp.x + w + 10.0, r1.max.y));
                    let shapes = [Shape::rect_filled(r, 12, CODE), Shape::rect_stroke(r, 12, Stroke::new(2.0, GUIDE), StrokeKind::Inside)];
                    if lifted.as_ref().map(|(b, _)| b.contains(&d.line)).unwrap_or(false) {
                        lift_bg.extend(shapes);
                    } else {
                        bg.extend(shapes);
                    }
                }
            }
        }

        // Rounded selection highlights.
        if let Some(cr) = te_out.as_ref().and_then(|o| o.cursor_range) {
            let (a, b) = (cr.primary.index.0.min(cr.secondary.index.0), cr.primary.index.0.max(cr.secondary.index.0));
            if a < b {
                let mut rs = 0usize;
                for row in &g.rows {
                    let n = row.char_count_excluding_newline().0;
                    let re = rs + n + usize::from(row.ends_with_newline);
                    if a < re.max(rs + 1) && b > rs {
                        let x = |i: usize| -> f32 {
                            let k = i - rs;
                            row.pos.x + row.glyphs.get(k).map(|gl| gl.pos.x).unwrap_or(row.size.x)
                        };
                        let xa = x(a.max(rs));
                        let xb = if b > rs + n { row.pos.x + row.size.x + 5.0 } else { x(b) };
                        if xb > xa {
                            let r = Rect::from_min_max(pos2(gp.x + xa - 1.0, gp.y + row.pos.y + 3.0), pos2(gp.x + xb + 1.0, gp.y + row.pos.y + row.size.y - 3.0));
                            sel_shapes.push(Shape::rect_filled(r, 6, SELECTION));
                        }
                    }
                    rs = re;
                }
            }
        }

        // The pill behind the line being edited, with the block handle at its right end.
        let pill_x0 = gp.x - 14.0;
        let pill_x1 = gp.x + w + 14.0;
        if !dragging {
            let r0t = ltop(cur_line.min(lines.len() - 1));
            let r1b = lbot(cur_line.min(lines.len() - 1));
            let ty = ctx.animate_value_with_time(id.with("pill-top"), r0t, 0.18);
            let th = ctx.animate_value_with_time(id.with("pill-h"), (r1b - r0t).max(LH), 0.18);
            let pill = Rect::from_min_max(pos2(pill_x0, gp.y + ty), pos2(pill_x1, gp.y + ty + th));
            bg.extend(pill_shapes(pill, 1.0));
            cursor_screen = Some(row_rect(post_cursor));
            let hr = handle_rect(pill_x1, gp.y + ty);
            handles.push((hr, cur_line, id.with("handle-active"), 1.0));
        }
        // Hovering another line shows a faint pill and its handle too.
        let hover_ptr = ctx.input(|i| i.pointer.hover_pos()).filter(|p| ui.clip_rect().contains(*p));
        if !dragging && e.palette.is_none() {
            if let Some(p) = hover_ptr {
                let area = Rect::from_min_max(pos2(pill_x0 - 20.0, gp.y), pos2(pill_x1, gp.y + g.size().y.max(LH)));
                let hl = if area.contains(p) { (0..lines.len()).find(|&li| !hidden_now.contains(&li) && p.y - gp.y >= ltop(li) && p.y - gp.y < lbot(li)) } else { None };
                let hid = id.with("handle-hover");
                // Keep the hovered handle while the pointer travels onto it.
                let held = e.hover_line.filter(|&l| l < lines.len() && handle_rect(pill_x1, gp.y + ltop(l)).expand(6.0).contains(p));
                if let Some(hl) = held.or(hl).filter(|&l| l != cur_line) {
                    e.hover_line = Some(hl);
                    let t = ctx.animate_bool_with_time(id.with(("hover-line", hl)), true, 0.12);
                    let pill = Rect::from_min_max(pos2(pill_x0, gp.y + ltop(hl)), pos2(pill_x1, gp.y + lbot(hl).max(ltop(hl) + LH)));
                    bg.extend(pill_shapes(pill, 0.45 * t));
                    handles.push((handle_rect(pill_x1, gp.y + ltop(hl)), hl, hid, t));
                } else {
                    e.hover_line = None;
                }
            }
        }
        // The dragged block floats above the rest on its own pill, with a soft shadow.
        if let Some((b, o)) = &lifted {
            let top = gp.y + ltop(b.start) + o;
            let bot = gp.y + lbot(b.end.saturating_sub(1).max(b.start)) + o;
            let r = Rect::from_min_max(pos2(pill_x0, top), pos2(pill_x1, bot.max(top + LH)));
            let sh = egui::epaint::Shadow { offset: [0, 10], blur: 28, spread: 0, color: Color32::from_black_alpha(220) };
            lift_bg.insert(0, sh.as_shape(r, 12).into());
            lift_bg.insert(1, Shape::rect_filled(r, 12, Color32::from_gray(0x17)));
            lift_bg.insert(2, Shape::rect_stroke(r, 12, Stroke::new(1.0, Color32::from_gray(0x38)), StrokeKind::Inside));
            if let Some(d) = &e.drag {
                handles.push((handle_rect(pill_x1, top), b.start, d.handle, 1.0));
            }
        }
        bg.extend(sel_shapes.drain(..));
        ui.painter().set(bg_idx, Shape::Vec(bg));

        // While dragging, paint the text in pieces, each at its own offset.
        if dragging {
            let mut li = 0;
            while li < lines.len() {
                let o = off[li];
                let lift = lifted.as_ref().map(|(b, _)| b.contains(&li)).unwrap_or(false);
                let mut lj = li + 1;
                while lj < lines.len() && (off[lj] - o).abs() < 0.25 && lifted.as_ref().map(|(b, _)| b.contains(&lj)).unwrap_or(false) == lift {
                    lj += 1;
                }
                let rows = line_rows[li].start..line_rows[lj - 1].end;
                if !rows.is_empty() {
                    let piece = hang::rows_galley(&g, rows);
                    let p = if lift { &lift_painter } else { &base_painter };
                    if lift {
                        p.extend(std::mem::take(&mut lift_bg));
                    }
                    p.galley(gp + vec2(0.0, o), piece, TEXT2);
                }
                li = lj;
            }
        }

        // Foreground decorations replace the hidden markers.
        for d in &decos {
            let r = row_rect(d.at);
            let cy = r.min.y + LH / 2.0;
            let painter = pnt(d.line);
            match d.kind {
                DecoKind::Dot => {
                    painter.circle_filled(pos2(r.min.x + 2.5, cy), 2.5, WHITE);
                }
                DecoKind::Box(done) => {
                    let b = Rect::from_min_size(pos2(r.min.x, cy - 8.0), vec2(16.0, 16.0));
                    let hov = ui.rect_contains_pointer(b.expand(3.0));
                    let t = ctx.animate_bool_with_time(id.with(("box", d.line)), done, 0.12);
                    let border = if hov { WHITE } else { widgets::lerp_color(MUTED, WHITE, t) };
                    painter.rect_filled(b, 4, WHITE.gamma_multiply(t));
                    painter.rect_stroke(b, 4, Stroke::new(2.0, border), StrokeKind::Inside);
                    if done {
                        icons::paint(painter, b.shrink(2.0), Icon::Check, BLACK);
                    }
                    if hov {
                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    boxes.push((b, d.line));
                }
                DecoKind::Bar => {
                    let r1 = row_rect(d.end);
                    painter.rect_filled(Rect::from_min_max(pos2(r.min.x, r.min.y + 2.0), pos2(r.min.x + 3.0, r1.max.y - 2.0)), 2, WHITE);
                }
                DecoKind::Rule => {
                    painter.line_segment([pos2(gp.x, cy), pos2(gp.x + w, cy)], Stroke::new(2.0, BORDER));
                }
                DecoKind::Code { .. } => {}
            }
        }

        // Fold chevrons beside foldable headings.
        for (li, closed) in heads {
            let painter = pnt(li);
            let r = row_rect(starts[li]);
            let cr = Rect::from_center_size(pos2(gp.x - 34.0 + 14.0, r.min.y + LH / 2.0), vec2(28.0, 28.0));
            let resp = ui.interact(cr, id.with(("chev", li)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let h = ctx.animate_bool_with_time(id.with(("chevh", li)), resp.hovered(), 0.12);
            if h > 0.0 {
                painter.circle_filled(cr.center(), 14.0, PILL.gamma_multiply(h));
            }
            let open = ctx.animate_bool_with_time_and_easing(id.with(("chevr", lines[li].trim())), !closed, 0.25, egui::emath::easing::back_out);
            let col = if closed || resp.hovered() { WHITE } else { MUTED };
            icons::paint_rot(painter, Rect::from_center_size(cr.center(), vec2(14.0, 14.0)), Icon::Chevron, col, open * std::f32::consts::FRAC_PI_2);
            if resp.clicked() {
                fold_toggle = Some(lines[li].trim().to_string());
            }
        }

        // Block handles: drag to move the block, click for its menu.
        let mut start_drag = None;
        for (hr, line, hid, t) in std::mem::take(&mut handles) {
            let resp = ui
                .interact(hr, hid, Sense::click_and_drag())
                .on_hover_cursor(if dragging { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
            let p = pnt(line);
            let hov = ctx.animate_bool_with_time(hid.with(("h", line)), resp.hovered() || dragging, 0.12);
            paint_handle(p, hr, t, hov);
            if resp.hovered() && !dragging {
                // Outline what moves with it.
                let blk = blocks::block_at(&lines, line);
                if blk.len() > 1 {
                    let r = Rect::from_min_max(pos2(pill_x0, gp.y + ltop(blk.start)), pos2(pill_x1, gp.y + lbot(blk.end - 1)));
                    p.rect_stroke(r, 12, Stroke::new(1.0, Color32::from_gray(0x3a)), StrokeKind::Inside);
                }
                widgets::tip(ui, &resp, "Drag to move · click for actions", "", true);
            }
            if !dragging && resp.drag_started() {
                let blk = blocks::block_at(&lines, line);
                let py = resp.interact_pointer_pos().map(|p| p.y).unwrap_or(hr.center().y);
                start_drag = Some(BlockDrag { block: blk.clone(), grab: py - (gp.y + ltop(blk.start)), handle: hid, serial: (now * 1000.0) as i64 });
            } else if !dragging && resp.clicked() {
                menu_req = Some((hr, line));
            }
        }
        if start_drag.is_some() {
            e.drag = start_drag;
            e.hover_line = None;
            ctx.request_repaint();
        }

        // The cursor never rests inside a folded section.
        if !dragging && hidden_now.contains(&cur_line) {
            let mut h = cur_line;
            while h > 0 && hidden_now.contains(&h) {
                h -= 1;
            }
            let end = section_end(&lines, h);
            let target = if post_cursor > e.snap.sel.1 && end < lines.len() { starts[end] } else { starts[h] + lines[h].chars().count() };
            set_cursor(&ctx, id, target, target);
            post_cursor = target;
            ctx.request_repaint();
        }

        if let Some(p) = &e.palette {
            pal_anchor = Some(match p.mode {
                PalMode::Slash(sl) => row_rect(sl),
                PalMode::Input { at, .. } => row_rect(at.1.min(acc.saturating_sub(1))),
            });
        }

        // Keep the line being edited clear of the toolbar.
        if let Some(cr) = cursor_screen.filter(|_| !dragging) {
            if (changed || e.scrolled_for != Some(post_cursor)) && (focused || new_sel.is_some()) {
                let clip = ui.clip_rect();
                if cr.max.y > clip.max.y - 90.0 {
                    ui.scroll_with_delta(vec2(0.0, -(cr.max.y - clip.max.y + 120.0)));
                } else if cr.min.y < clip.min.y + 10.0 {
                    ui.scroll_with_delta(vec2(0.0, clip.min.y - cr.min.y + 40.0));
                }
            }
        }
        e.scrolled_for = Some(post_cursor);

        // Clicking below the text puts the cursor at the end.
        let tail = ui.allocate_response(vec2(avail, 240.0), Sense::click());
        if tail.clicked() {
            let n = e.text.chars().count();
            set_cursor(&ctx, id, n, n);
            ctx.memory_mut(|m| m.request_focus(id));
        }
        if tail.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::Text);
        }
    });
    e.boxes = boxes;

    // Drop a dragged block into its new place.
    if let Some((blk, gap, home)) = drop_at {
        e.drag = None;
        if gap != home {
            let text = e.text.clone();
            let lines: Vec<&str> = text.split('\n').collect();
            let (hidden, _) = fold_info(&lines, &folds);
            let us = blocks::units(&lines, &blk, &hidden);
            let (t, _, order) = blocks::move_block(&lines, &blk, &us, gap);
            let s0 = e.last_sel.unwrap_or((post_cursor, post_cursor));
            let sel2 = (remap_cursor(&lines, &order, s0.0), remap_cursor(&lines, &order, s0.1));
            e.replace_text(&ctx, t, sel2);
            new_sel = Some(sel2);
            changed = true;
            force_hist = true;
            // replace_text already recorded the undo step.
            e.snap = Snap { text: e.text.clone(), sel: sel2 };
        } else {
            ctx.memory_mut(|m| m.request_focus(id));
        }
        ctx.request_repaint();
    }

    // Tick a checkbox and put the cursor back where it was.
    if let Some(line) = tick {
        let mut lines: Vec<String> = e.text.split('\n').map(String::from).collect();
        if let Some(l) = lines.get_mut(line) {
            let c = md::classify(l);
            if let Block::Task(done) = c.block {
                let at = c.marker.start + 3;
                l.replace_range(at..at + 1, if done { " " } else { "x" });
                e.text = lines.join("\n");
                changed = true;
                force_hist = true;
                if let Some((a, b)) = sel.or(e.last_sel) {
                    set_cursor(&ctx, id, a, b);
                }
            }
        }
    }

    // Open the slash menu after typing `/` at a line start or after a space.
    if changed && e.palette.is_none() && new_sel.is_none() && post_cursor > 0 {
        let b = byte_at(&e.text, post_cursor);
        let (ls, _) = line_bounds(&e.text, b);
        // The slash may be followed by a few characters typed in the same frame.
        let typed_from = byte_at(&e.text, cursor.min(post_cursor)).max(ls);
        if let Some(rel) = e.text[typed_from..b].rfind('/') {
            let sb = typed_from + rel;
            let prev = e.text[..sb].chars().last();
            let fences = e.text[..sb].lines().filter(|l| md::classify(l).block == Block::Fence).count();
            let ticks = e.text[ls..sb].matches('`').count();
            let tail = &e.text[sb + 1..b];
            if matches!(prev, None | Some(' ') | Some('\n') | Some('\t')) && fences % 2 == 0 && ticks % 2 == 0 && !tail.contains(' ') {
                e.palette = Some(Palette::new(PalMode::Slash(char_at(&e.text, sb)), now));
            }
        }
    }
    // Close it when the cursor leaves the query, or the query reads as plain text.
    if let Some(p) = &mut e.palette {
        match &mut p.mode {
            PalMode::Slash(slash) => {
                let slash = *slash;
                let q: Option<String> = if post_cursor > slash && e.text.chars().nth(slash) == Some('/') {
                    let q: String = e.text.chars().skip(slash + 1).take(post_cursor - slash - 1).collect();
                    if q.contains('\n') || q.contains('/') || q.starts_with(' ') || q.chars().count() > 24 || matches(&q).is_empty() {
                        None
                    } else {
                        Some(q)
                    }
                } else {
                    None
                };
                match q {
                    None => e.palette = None,
                    Some(q) => {
                        if q != p.lastq {
                            p.sel = 0;
                            p.lastq = q;
                        }
                    }
                }
            }
            PalMode::Input { .. } => {
                // Clicking back into the text closes it.
                if focused && now - p.opened > 0.2 && new_sel.is_none() {
                    e.palette = None;
                }
            }
        }
    }

    // Floating toolbar.
    let (blk, isb, isi, iss) = styles_at(&e.text, post_cursor);
    let tools = vec![
        Tool::Btn(Err("H"), Cmd::HeadingCycle, matches!(blk, Block::Heading(_)), "Heading", format!("{MOD} ⌥ 1-3")),
        Tool::Btn(Err("B"), Cmd::Bold, isb, "Bold", format!("{MOD} B")),
        Tool::Btn(Err("I"), Cmd::Italic, isi && !isb, "Italic", format!("{MOD} I")),
        Tool::Btn(Err("S"), Cmd::Strike, iss, "Strikethrough", format!("{MOD} ⇧ X")),
        Tool::Btn(Ok(Icon::Code), Cmd::Code, false, "Code", format!("{MOD} `")),
        Tool::Btn(Ok(Icon::Link), Cmd::Link, false, "Link", format!("{MOD} K")),
        Tool::Sep,
        Tool::Btn(Ok(Icon::Bullets), Cmd::Bullet, blk == Block::Bullet, "Bullet list", format!("{MOD} ⇧ 8")),
        Tool::Btn(Ok(Icon::Numbers), Cmd::Number, blk == Block::Number, "Numbered list", format!("{MOD} ⇧ 7")),
        Tool::Btn(Ok(Icon::Checkbox), Cmd::Task, matches!(blk, Block::Task(_)), "Checkbox", format!("{MOD} ⇧ 9")),
        Tool::Btn(Ok(Icon::Quote), Cmd::Quote, blk == Block::Quote, "Quote", format!("{MOD} ⇧ .")),
        Tool::Btn(Ok(Icon::Rule), Cmd::Divider, false, "Divider", "---".into()),
        Tool::Sep,
        Tool::Btn(Ok(Icon::Indent), Cmd::Indent, false, "Indent", "Tab".into()),
        Tool::Btn(Ok(Icon::Outdent), Cmd::Outdent, false, "Outdent", "⇧ Tab".into()),
        Tool::Sep,
        Tool::Slash,
    ];
    let slash_glyph = ctx.fonts_mut(|f| f.layout_no_wrap("/".into(), bold(16.0), WHITE).size().x);
    let insert_w = ctx.fonts_mut(|f| f.layout_no_wrap("Insert".into(), semibold(12.0), MUTED).size().x);
    let full_slash = 12.0 + slash_glyph + 6.0 + insert_w + 12.0;
    let nbtn = tools.iter().filter(|t| matches!(t, Tool::Btn(..))).count() as f32;
    let nsep = tools.iter().filter(|t| matches!(t, Tool::Sep)).count() as f32;
    let full = 12.0 + nbtn * 42.0 + nsep * 12.0 + full_slash;
    let avail = rect.width() - 32.0;
    // Narrow windows: drop the "Insert" label first, then shrink the buttons.
    let compact = full > avail;
    let slash_w = if compact { 40.0 } else { full_slash };
    let bw = if compact { ((avail - 12.0 - nsep * 12.0 - slash_w) / nbtn).clamp(30.0, 42.0) } else { 42.0 };
    let total = 12.0 + nbtn * bw + nsep * 12.0 + slash_w;
    let bar_w = total.min(avail.max(120.0));
    let bar = Rect::from_center_size(pos2(rect.center().x, rect.max.y - 18.0 - 26.0), vec2(bar_w, 52.0));
    let layer = egui::LayerId::new(egui::Order::Middle, Id::new("toolbar"));
    let mut tb = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(bar));
    tb.set_clip_rect(bar);
    let off = 0.0;
    let fg = tb.painter().clone();
    fg.rect_filled(bar, 26, TOOLBAR);
    let mut x = bar.min.x + 6.0 - off;
    for (i, t) in tools.into_iter().enumerate() {
        match t {
            Tool::Sep => {
                fg.rect_filled(Rect::from_min_size(pos2(x + 5.0, bar.center().y - 11.0), vec2(2.0, 22.0)), 1, GUIDE);
                x += 12.0;
            }
            Tool::Btn(icon, cmd, on, tip, key) => {
                let r = Rect::from_min_size(pos2(x, bar.min.y + 5.0 + (42.0 - bw) / 2.0), vec2(bw, bw));
                x += bw;
                let hit = r.intersect(bar);
                if hit.width() <= 0.0 {
                    continue;
                }
                let tid = Id::new(("tool", i));
                let resp = tb.interact(hit, tid, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                widgets::tip(&tb, &resp, tip, &key, true);
                let h = ctx.animate_bool_with_time(tid.with("h"), resp.hovered(), 0.12);
                let pr = ctx.animate_bool_with_time(tid.with("p"), resp.is_pointer_button_down_on(), 0.1);
                let onv = ctx.animate_bool_with_time(tid.with("on"), on, 0.12);
                let scale = 1.0 - 0.08 * pr;
                if h > 0.0 {
                    fg.circle_filled(r.center(), bw / 2.0 * scale, TOOLBAR_HOVER.gamma_multiply(h));
                }
                if onv > 0.0 {
                    fg.circle_filled(r.center(), bw / 2.0 * scale, WHITE.gamma_multiply(onv));
                }
                let col = widgets::lerp_color(WHITE, BLACK, onv);
                match icon {
                    Ok(ic) => icons::paint(&fg, Rect::from_center_size(r.center(), vec2(21.0, 21.0) * scale), ic, col),
                    Err(t) => {
                        let font = if t == "I" { italic(17.0 * scale) } else { xbold(17.0 * scale) };
                        fg.text(r.center(), Align2::CENTER_CENTER, t, font, col);
                        if t == "S" {
                            fg.line_segment([r.center() - vec2(6.5, -0.5) * scale, r.center() + vec2(6.5, 0.5) * scale], Stroke::new(1.8, col));
                        }
                    }
                }
                if resp.clicked() {
                    toolbar_cmd = Some(cmd);
                }
            }
            Tool::Slash => {
                let r = Rect::from_min_size(pos2(x, bar.min.y + 5.0), vec2(slash_w, 42.0));
                x += slash_w;
                let hit = r.intersect(bar);
                if hit.width() <= 0.0 {
                    continue;
                }
                let resp = tb.interact(hit, Id::new("tool-slash"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                widgets::tip(&tb, &resp, "All formatting", &format!("{MOD} /"), true);
                let h = ctx.animate_bool_with_time(Id::new("tool-slash-h"), resp.hovered(), 0.12);
                let pr = ctx.animate_bool_with_time(Id::new("tool-slash-p"), resp.is_pointer_button_down_on(), 0.1);
                let rr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.08 * pr));
                if h > 0.0 {
                    fg.rect_filled(rr, 21, TOOLBAR_HOVER.gamma_multiply(h));
                }
                if compact {
                    fg.text(r.center(), Align2::CENTER_CENTER, "/", bold(16.0), WHITE);
                } else {
                    fg.text(pos2(r.min.x + 12.0, r.center().y), Align2::LEFT_CENTER, "/", bold(16.0), WHITE);
                }
                if !compact {
                    fg.text(pos2(r.min.x + 12.0 + slash_glyph + 6.0, r.center().y), Align2::LEFT_CENTER, "Insert", semibold(12.0), MUTED);
                } else {
                    let _ = slash_glyph;
                }
                if resp.clicked() {
                    open_input = true;
                }
            }
        }
    }

    // Formatting palette.
    if let (Some(p), Some(anchor)) = (&mut e.palette, pal_anchor) {
        let input = p.is_input();
        let q: String = match &p.mode {
            PalMode::Slash(sl) => e.text.chars().skip(sl + 1).take(post_cursor.saturating_sub(sl + 1)).collect(),
            PalMode::Input { query, .. } => query.clone(),
        };
        let list = matches(&q);
        if p.sel >= list.len() {
            p.sel = list.len().saturating_sub(1);
        }
        let t = widgets::progress(&ctx, p.opened, 0.14);
        let ease = widgets::ease_pop(t);
        let screen = ctx.content_rect();
        let wpal = 320.0;
        let list_h = if list.is_empty() { 44.0 } else { (list.len() as f32 * 46.0).min(300.0) };
        let h = if input { 46.0 } else { 0.0 } + 30.0 + list_h + 46.0 + 12.0;
        let mut pos = pos2(anchor.min.x - 12.0, anchor.max.y + 8.0);
        let above = pos.y + h > screen.max.y - 10.0;
        if above {
            pos.y = anchor.min.y - h - 8.0;
        }
        pos.x = pos.x.clamp(screen.min.x + 8.0, screen.max.x - wpal - 12.0);
        pos.y = pos.y.max(8.0) + (1.0 - ease) * if above { 4.0 } else { -4.0 };
        let mut hover_sel = None;
        egui::Area::new(Id::new("palette")).order(egui::Order::Foreground).fixed_pos(pos).show(&ctx, |ui| {
            ui.multiply_opacity(t.min(1.0));
            egui::Frame::new().fill(MENU).stroke(Stroke::new(LINE, BORDER)).corner_radius(18).inner_margin(6).show(ui, |ui| {
                ui.set_width(wpal - 12.0);
                if let PalMode::Input { query, focus, .. } = &mut p.mode {
                    let (r, _) = ui.allocate_exact_size(vec2(wpal - 12.0, 40.0), Sense::hover());
                    ui.painter().text(pos2(r.min.x + 10.0, r.center().y - 1.0), Align2::LEFT_CENTER, "/", mono(14.0), MUTED);
                    let te = ui.put(
                        Rect::from_min_max(pos2(r.min.x + 28.0, r.min.y + 9.0), pos2(r.max.x - 10.0, r.max.y - 11.0)),
                        egui::TextEdit::singleline(query)
                            .id(Id::new("palq"))
                            .font(sans(14.0))
                            .text_color(WHITE)
                            .hint_text(egui::RichText::new("Type what you want, like todo or line").font(sans(14.0)).color(Color32::from_gray(0x66)))
                            .frame(egui::Frame::NONE)
                            .margin(egui::Margin::ZERO)
                            .desired_width(f32::INFINITY),
                    );
                    if *focus {
                        te.request_focus();
                        *focus = false;
                    }
                    if te.changed() {
                        p.sel = 0;
                        p.scroll = true;
                    }
                    ui.painter().line_segment([pos2(r.min.x, r.max.y), pos2(r.max.x, r.max.y)], Stroke::new(2.0, DIVIDER));
                    ui.add_space(6.0);
                }
                let (r, _) = ui.allocate_exact_size(vec2(wpal - 12.0, 30.0), Sense::hover());
                let mut tj = LayoutJob::default();
                tj.append("INSERT", 0.0, TextFormat { font_id: bold(11.0), color: MUTED, extra_letter_spacing: 1.0, ..Default::default() });
                let tg = ui.painter().layout_job(tj);
                ui.painter().galley(pos2(r.min.x + 10.0, r.center().y - tg.size().y / 2.0), tg, MUTED);
                if !q.is_empty() {
                    ui.painter().text(pos2(r.max.x - 10.0, r.center().y), Align2::RIGHT_CENTER, format!("/{q}"), mono(11.0), TEXT3);
                }
                if list.is_empty() {
                    let (r, _) = ui.allocate_exact_size(vec2(wpal - 12.0, 44.0), Sense::hover());
                    ui.painter().text(pos2(r.min.x + 10.0, r.center().y), Align2::LEFT_CENTER, format!("Nothing matches “{q}”."), sans(13.0), MUTED);
                }
                egui::ScrollArea::vertical().id_salt("pal-list").max_height(300.0).show(ui, |ui| {
                    for (i, c) in list.iter().enumerate() {
                        let (r, resp) = ui.allocate_exact_size(vec2(wpal - 12.0, 46.0), Sense::click());
                        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                        if resp.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
                            hover_sel = Some(i);
                        }
                        let on = i == p.sel;
                        if on {
                            ui.painter().rect_filled(r, 12, WHITE);
                            if p.scroll {
                                ui.scroll_to_rect(r, None);
                            }
                        }
                        let fgc = if on { BLACK } else { WHITE };
                        let ic = Rect::from_min_size(pos2(r.min.x + 10.0, r.center().y - 16.0), vec2(32.0, 32.0));
                        ui.painter().rect_stroke(ic, 9, Stroke::new(LINE, if on { Color32::from_gray(0xcc) } else { BORDER }), StrokeKind::Inside);
                        match c.icon {
                            Ok(icn) => icons::paint(ui.painter(), Rect::from_center_size(ic.center(), vec2(18.0, 18.0)), icn, fgc),
                            Err(t) => {
                                ui.painter().text(ic.center(), Align2::CENTER_CENTER, t, if t == "I" { italic(13.0) } else { xbold(13.0) }, fgc);
                                if t == "S" {
                                    ui.painter().line_segment([ic.center() - vec2(5.0, -0.5), ic.center() + vec2(5.0, 0.5)], Stroke::new(1.5, fgc));
                                }
                            }
                        }
                        ui.painter().text(pos2(r.min.x + 54.0, r.center().y - 7.0), Align2::LEFT_CENTER, c.name, semibold(14.0), fgc);
                        ui.painter().text(pos2(r.min.x + 54.0, r.center().y + 9.0), Align2::LEFT_CENTER, c.desc, sans(11.0), if on { Color32::from_gray(0x55) } else { MUTED });
                        ui.painter().text(pos2(r.max.x - 10.0, r.center().y), Align2::RIGHT_CENTER, c.syntax, mono(11.0), if on { Color32::from_gray(0x55) } else { SYNTAX });
                        if resp.clicked() {
                            pal_click = Some(c.cmd);
                        }
                    }
                });
                ui.add_space(6.0);
                let (r, _) = ui.allocate_exact_size(vec2(wpal - 12.0, 34.0), Sense::hover());
                let pnt = ui.painter();
                pnt.line_segment([pos2(r.min.x, r.min.y + 1.0), pos2(r.max.x, r.min.y + 1.0)], Stroke::new(2.0, DIVIDER));
                let cy = r.center().y + 3.0;
                let mut cx = r.min.x + 10.0;
                cx += chip(pnt, cx, cy, "Tab") + 4.0;
                cx += chip(pnt, cx, cy, "↑↓") + 5.0;
                cx += pnt.text(pos2(cx, cy), Align2::LEFT_CENTER, "move", sans(11.0), MUTED).width() + 12.0;
                cx += chip(pnt, cx, cy, "Enter") + 5.0;
                cx += pnt.text(pos2(cx, cy), Align2::LEFT_CENTER, "insert", sans(11.0), MUTED).width() + 12.0;
                cx += chip(pnt, cx, cy, "Esc") + 5.0;
                pnt.text(pos2(cx, cy), Align2::LEFT_CENTER, if input { "close" } else { "keep a plain /" }, sans(11.0), MUTED);
            });
        });
        p.scroll = false;
        if let Some(i) = hover_sel {
            p.sel = i;
        }
    }

    // Toolbar and palette clicks act on the last selection and give focus back.
    let base = sel.or(e.last_sel).unwrap_or((post_cursor, post_cursor));
    if let Some(c) = pal_click {
        if let Some(p) = e.palette.take() {
            let (a, b) = match p.mode {
                PalMode::Slash(slash) => {
                    let (b0, b1) = (byte_at(&e.text, slash), byte_at(&e.text, post_cursor.max(slash + 1)));
                    e.hist.push(Snap { text: e.text.clone(), sel: (post_cursor, post_cursor) });
                    e.fut.clear();
                    e.text.replace_range(b0..b1, "");
                    e.snap = Snap { text: e.text.clone(), sel: (slash, slash) };
                    apply(&mut e.text, (slash, slash), c, true)
                }
                PalMode::Input { at, .. } => apply(&mut e.text, at, c, true),
            };
            set_cursor(&ctx, id, a, b);
            ctx.memory_mut(|m| m.request_focus(id));
            changed = true;
            force_hist = true;
        }
    }
    if let Some(c) = toolbar_cmd {
        let (a, b) = apply(&mut e.text, base, c, false);
        set_cursor(&ctx, id, a, b);
        ctx.memory_mut(|m| m.request_focus(id));
        changed = true;
        force_hist = true;
    }
    if open_input && e.palette.is_none() {
        e.palette = Some(Palette::new(PalMode::Input { query: String::new(), at: base, focus: true }, now));
    }

    if !deferred.is_empty() {
        e.pending = deferred;
        ctx.request_repaint();
    }
    if changed {
        e.dirty_since = Some(now);
        ctx.request_repaint();
    }

    // Undo history: one step per pause in typing, per word, and per command.
    let end_sel = egui::text_edit::TextEditState::load(&ctx, id)
        .and_then(|s| s.cursor.char_range())
        .map(|r| (r.secondary.index.0, r.primary.index.0))
        .unwrap_or((post_cursor, post_cursor));
    if e.text != e.snap.text {
        let pc = end_sel.1;
        let ws = pc > 0 && e.text.chars().nth(pc - 1).map(|c| c.is_whitespace()).unwrap_or(false);
        if force_hist || ws || now - e.last_push >= 0.9 {
            e.hist.push(e.snap.clone());
            if e.hist.len() > 200 {
                e.hist.remove(0);
            }
            e.fut.clear();
            e.last_push = if force_hist || ws { 0.0 } else { now };
        }
        e.snap.text = e.text.clone();
    }
    e.snap.sel = end_sel;
    e.had_focus = ctx.memory(|m| m.has_focus(id));

    if let Some((hr, line)) = menu_req {
        use crate::app::{mi, sep, Action, MenuAt};
        let b = |op| Action::Block(op, line);
        let items = vec![
            mi(Icon::Cut, "Cut", "", b(BlockOp::Cut)),
            mi(Icon::Copy, "Copy", "", b(BlockOp::Copy)),
            mi(Icon::Duplicate, "Duplicate", "", b(BlockOp::Duplicate)),
            sep(),
            mi(Icon::ArrowUp, "Move up", "Alt ↑", b(BlockOp::Up)),
            mi(Icon::ArrowDown, "Move down", "Alt ↓", b(BlockOp::Down)),
            sep(),
            mi(Icon::Trash, "Delete", "", b(BlockOp::Delete)),
        ];
        app.open_menu(ui, MenuAt::Below(hr), items);
    }

    if let Some(k) = fold_toggle {
        let f = app.folds.entry(path).or_default();
        if !f.remove(&k) {
            f.insert(k);
        }
        store::save_folds(&app.folds);
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(t: &str, s: (usize, usize), c: Cmd) -> (String, (usize, usize)) {
        let mut t = t.to_string();
        let r = apply(&mut t, s, c, false);
        (t, r)
    }
    #[test]
    fn blocks() {
        assert_eq!(run("hello", (2, 2), Cmd::Bullet).0, "- hello");
        assert_eq!(run("- hello", (4, 4), Cmd::Bullet).0, "hello");
        assert_eq!(run("- hello", (4, 4), Cmd::Task).0, "- [ ] hello");
        assert_eq!(run("a\nb", (0, 3), Cmd::Number).0, "1. a\n2. b");
        assert_eq!(run("# a", (2, 2), Cmd::HeadingCycle).0, "## a");
        assert_eq!(run("x", (1, 1), Cmd::Indent), ("  x".into(), (3, 3)));
        assert_eq!(run("  x", (3, 3), Cmd::Outdent), ("x".into(), (1, 1)));
    }
    #[test]
    fn inline() {
        assert_eq!(run("ab", (0, 2), Cmd::Bold), ("**ab**".into(), (2, 4)));
        assert_eq!(run("**ab**", (2, 4), Cmd::Bold), ("ab".into(), (0, 2)));
        assert_eq!(run("", (0, 0), Cmd::Link), ("[link text](https://)".into(), (1, 10)));
        assert_eq!(run("1. a\nb", (6, 6), Cmd::Number).0, "1. a\n2. b");
    }
    #[test]
    fn enter() {
        let mut t = "- a".to_string();
        assert_eq!(enter_continues(&mut t, 3), Some(6));
        assert_eq!(t, "- a\n- ");
        let mut t = "- a\n- ".to_string();
        enter_continues(&mut t, 6);
        assert_eq!(t, "- a\n");
        let mut t = "9. x".to_string();
        enter_continues(&mut t, 4);
        assert_eq!(t, "9. x\n10. ");
    }
    #[test]
    fn menu() {
        assert_eq!(matches("bulet")[0].name, "Bullet list");
        assert_eq!(matches("todo")[0].name, "Checkbox");
        assert_eq!(matches("big header")[0].name, "Heading 1");
        assert!(matches("zzzz").is_empty());
    }
}
