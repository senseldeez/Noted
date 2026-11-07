//! Live-preview editor: markdown is styled in place, syntax only shows on the line being
//! edited, which sits on a rounded grey pill. Slash menu, toolbar and list shortcuts.
use crate::app::{Action, App};
use crate::icons::{self, Icon};
use crate::md::{self, Block};
use crate::reader::{self, format_for};
use crate::store::{self, Item};
use crate::theme::*;
use crate::widgets::MOD;
use egui::text::{CCursor, CCursorRange, LayoutJob};
use egui::{pos2, vec2, Align2, Color32, FontId, Id, Key, Modifiers, Rect, Sense, Shape, Stroke, StrokeKind, TextFormat, Ui};
use std::path::PathBuf;

const LH: f32 = 28.0;

pub struct Palette {
    /// Char index of the `/` that opened it.
    pub slash: usize,
    pub sel: usize,
    /// The slash was typed for us (Ctrl+/ or the toolbar), so Esc removes it.
    pub inserted: bool,
}

pub struct Editor {
    pub path: PathBuf,
    pub title: String,
    saved_title: String,
    pub text: String,
    saved_text: String,
    pub dirty_since: Option<f64>,
    pub palette: Option<Palette>,
    pub focus_title: bool,
    pub is_new: bool,
    focus_body: bool,
    last_sel: Option<(usize, usize)>,
    boxes: Vec<(Rect, usize)>,
    /// Input that arrived after a key we handled, replayed next frame so keys apply in order.
    pending: Vec<egui::Event>,
}

impl Editor {
    pub fn new(item: &Item) -> Self {
        Editor {
            path: item.path.clone(),
            title: item.name.clone(),
            saved_title: item.name.clone(),
            text: item.text.clone(),
            saved_text: item.text.clone(),
            dirty_since: None,
            palette: None,
            focus_title: false,
            is_new: false,
            focus_body: true,
            last_sel: None,
            boxes: vec![],
            pending: vec![],
        }
    }

    pub fn write_text(&mut self) -> Result<(), String> {
        self.dirty_since = None;
        if self.text != self.saved_text {
            store::write(&self.path, &self.text).map_err(|e| format!("Couldn't save: {e}"))?;
            self.saved_text = self.text.clone();
        }
        Ok(())
    }

    /// Write text and apply a title change. Returns the new path when the file was renamed.
    pub fn save(&mut self) -> Result<Option<PathBuf>, String> {
        self.write_text()?;
        let t = self.title.trim().to_string();
        if !t.is_empty() && t != self.saved_title {
            match store::rename(&self.path, &t) {
                Some(new) => {
                    self.path = new.clone();
                    self.saved_title = t;
                    return Ok(Some(new));
                }
                None => {
                    self.title = self.saved_title.clone();
                    return Err("A note with that name already exists".into());
                }
            }
        }
        Ok(None)
    }
}

/* ---------- text helpers (char indices) ---------- */

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
                text.replace_range(ba..bb, "[text](url)");
                (a + 1, a + 5)
            } else {
                let s = format!("[{inner}](url)");
                text.replace_range(ba..bb, &s);
                let n = inner.chars().count();
                (a + n + 3, a + n + 6)
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
            let mut n = 0;
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
        Block::Quote => format!("{}> ", &line[..c.indent]),
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
        // Empty item: drop the marker (outdent first if nested).
        if c.indent > 0 {
            let cut = if line.starts_with('\t') { 1 } else { c.indent.min(2) };
            text.replace_range(ls..ls + cut, "");
            return Some(at - cut);
        }
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

/// Style the whole note. `active` is the line holding the cursor.
fn build(text: &str, active: Option<usize>, wrap: f32) -> (LayoutJob, Vec<Deco>) {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let mut decos = Vec::new();
    let hidden = fmt(sans(1.0), Color32::TRANSPARENT);
    let mut chars = 0usize;
    let mut in_code = false;
    let mut code_start: Option<usize> = None;
    let all: Vec<&str> = text.split('\n').collect();
    let n = all.len();
    for (li, line) in all.iter().enumerate() {
        let is_active = active == Some(li);
        let lc = line.chars().count();
        let c = md::classify(line);
        let nl_fmt;
        if c.block == Block::Fence || in_code {
            let opening = c.block == Block::Fence && !in_code;
            let closing = c.block == Block::Fence && in_code;
            if opening {
                in_code = true;
                code_start = Some(decos.len());
            }
            let color = if c.block == Block::Fence { SYNTAX } else { TEXT2 };
            let f = fmt(mono(13.0), color);
            job.append(line, 0.0, f.clone());
            decos.push(Deco { line: li, at: chars, end: chars + lc, kind: DecoKind::Code { first: opening, last: closing || li == n - 1 } });
            if closing {
                in_code = false;
                code_start = None;
            }
            nl_fmt = f;
        } else {
            let (base, color) = match c.block {
                Block::Heading(l) => (heading_font(l), WHITE),
                Block::Bullet | Block::Number | Block::Task(_) => (sans(16.0), WHITE),
                Block::Quote if !is_active => (italic(16.0), TEXT3),
                _ => (sans(16.0), if is_active { WHITE } else { TEXT2 }),
            };
            let done = matches!(c.block, Block::Task(true));
            // Indent
            if c.indent > 0 {
                // Widen indent spaces so each nesting level steps in like the reader.
                let mut f = fmt(sans(16.0), color);
                f.extra_letter_spacing = 9.5;
                job.append(&line[..c.indent], 0.0, f);
            }
            // Block marker
            let marker = &line[c.marker.clone()];
            if !marker.is_empty() || c.block == Block::Rule {
                let at = chars + line[..c.marker.start].chars().count();
                if is_active {
                    job.append(marker, 0.0, fmt(base.clone(), SYNTAX));
                } else {
                    match c.block {
                        Block::Heading(_) => job.append(marker, 0.0, hidden.clone()),
                        Block::Number => job.append(marker, 0.0, fmt(sans(16.0), MUTED)),
                        Block::Bullet => {
                            job.append(marker, 0.0, fmt(base.clone(), Color32::TRANSPARENT));
                            decos.push(Deco { line: li, at, end: chars + lc, kind: DecoKind::Dot });
                        }
                        Block::Task(d) => {
                            let mut f = fmt(base.clone(), Color32::TRANSPARENT);
                            f.extra_letter_spacing = -2.0;
                            job.append(marker, 0.0, f);
                            decos.push(Deco { line: li, at, end: chars + lc, kind: DecoKind::Box(d) });
                        }
                        Block::Quote => {
                            job.append(marker, 0.0, fmt(base.clone(), Color32::TRANSPARENT));
                            decos.push(Deco { line: li, at, end: chars + lc, kind: DecoKind::Bar });
                        }
                        Block::Rule => {
                            decos.push(Deco { line: li, at, end: chars + lc, kind: DecoKind::Rule });
                        }
                        _ => job.append(marker, 0.0, fmt(base.clone(), SYNTAX)),
                    }
                }
            }
            // Content
            if c.block == Block::Rule {
                let f = if is_active { fmt(base.clone(), SYNTAX) } else { fmt(base.clone(), Color32::TRANSPARENT) };
                job.append(&line[c.indent..], 0.0, f);
            } else {
                for sp in md::inline(line, c.content.clone()) {
                    let t = &line[sp.range.clone()];
                    if sp.style.marker {
                        if is_active {
                            job.append(t, 0.0, fmt(base.clone(), SYNTAX));
                        } else {
                            job.append(t, 0.0, hidden.clone());
                        }
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
        chars += lc;
        if li + 1 < n {
            job.append("\n", 0.0, nl_fmt);
            chars += 1;
        }
    }
    // An unclosed fence runs to the end of the note.
    if let Some(i) = code_start {
        if let Some(d) = decos.last_mut() {
            if let DecoKind::Code { last, .. } = &mut d.kind {
                *last = true;
            }
        }
        let _ = i;
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

pub fn show(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let Some(e) = app.editor.as_mut() else { return };
    let id = editor_id(&e.path);
    let title_id = Id::new(("title", &e.path));
    let rect = ui.max_rect();
    let focused = ctx.memory(|m| m.has_focus(id));
    let state = egui::text_edit::TextEditState::load(&ctx, id);
    let sel = state.as_ref().and_then(|s| s.cursor.char_range()).map(|r| (r.secondary.index.0, r.primary.index.0));
    if focused {
        if let Some(s) = sel {
            e.last_sel = Some(s);
        }
    }
    let cursor = sel.map(|s| s.1).or(e.last_sel.map(|s| s.1)).unwrap_or(0);
    let mut new_sel: Option<(usize, usize)> = None;
    let mut changed = false;

    // A click on a checkbox ticks it without moving the cursor.
    let mut tick: Option<usize> = None;
    if let Some(p) = ctx.input(|i| if i.pointer.primary_pressed() { i.pointer.interact_pos() } else { None }) {
        if let Some((_, line)) = e.boxes.iter().find(|(r, _)| r.expand(3.0).contains(p)) {
            tick = Some(*line);
        }
    }

    // Keys the editor handles itself, before the text field sees them.
    let mut cursor = cursor;
    let mut sel = sel;
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
                    matches!(key, Key::Enter | Key::Tab | Key::ArrowUp | Key::ArrowDown | Key::Escape) || modifiers.command
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
                        e.palette = Some(Palette { slash, sel: 0, inserted: false });
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
        if let Some(p) = &mut e.palette {
            let q_end = cursor;
            let q: String = e.text.chars().skip(p.slash + 1).take(q_end.saturating_sub(p.slash + 1)).collect();
            let list = matches(&q);
            let (up, down, tab, stab, enter, esc) = ctx.input_mut(|i| {
                (
                    i.consume_key(Modifiers::NONE, Key::ArrowUp),
                    i.consume_key(Modifiers::NONE, Key::ArrowDown),
                    i.consume_key(Modifiers::NONE, Key::Tab),
                    i.consume_key(Modifiers::SHIFT, Key::Tab),
                    i.consume_key(Modifiers::NONE, Key::Enter),
                    i.consume_key(Modifiers::NONE, Key::Escape),
                )
            });
            if !list.is_empty() {
                if down || tab {
                    p.sel = (p.sel + 1) % list.len()
                }
                if up || stab {
                    p.sel = (p.sel + list.len() - 1) % list.len()
                }
            }
            if esc {
                if p.inserted {
                    let b = byte_at(&e.text, p.slash);
                    e.text.replace_range(b..b + 1, "");
                    new_sel = Some((cursor - 1, cursor - 1));
                    changed = true;
                }
                e.palette = None;
            } else if enter {
                if let Some(c) = list.get(p.sel.min(list.len().saturating_sub(1))) {
                    let (b0, b1) = (byte_at(&e.text, p.slash), byte_at(&e.text, q_end));
                    e.text.replace_range(b0..b1, "");
                    new_sel = Some(apply(&mut e.text, (p.slash, p.slash), c.cmd, true));
                    changed = true;
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
            } else if k(cmd, Key::Slash) {
                let b = byte_at(&e.text, s.1);
                e.text.insert(b, '/');
                e.palette = Some(Palette { slash: s.1, sel: 0, inserted: true });
                new_sel = Some((s.1 + 1, s.1 + 1));
                changed = true;
            } else if s.0 == s.1 && ctx.input(|i| i.key_pressed(Key::Enter) && i.modifiers.is_none()) {
                let mut t = e.text.clone();
                if let Some(c) = enter_continues(&mut t, s.1) {
                    ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                    e.text = t;
                    new_sel = Some((c, c));
                    changed = true;
                }
            }
        }
    }
    if let Some((a, b)) = new_sel {
        set_cursor(&ctx, id, a, b);
    }
    let active = if focused || new_sel.is_some() {
        Some(line_index(&e.text, byte_at(&e.text, new_sel.map(|s| s.1).unwrap_or(cursor))))
    } else {
        None
    };

    let mut toolbar_cmd: Option<Cmd> = None;
    let mut open_palette_from_toolbar = false;
    let mut pal_click: Option<Cmd> = None;
    let mut cursor_screen: Option<Rect> = None;
    let mut boxes = Vec::new();
    let mut title_enter = false;

    egui::ScrollArea::vertical().id_salt(("editor-scroll", &e.path)).auto_shrink([false, false]).show(ui, |ui| {
        let avail = ui.available_width();
        let (left, w) = reader::column(avail);
        let x0 = ui.max_rect().min.x + left;
        ui.add_space(8.0);
        // Title
        let top = ui.cursor().min.y;
        let title_resp = ui
            .scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(x0, top), vec2(w, 200.0))), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut e.title)
                        .id(title_id)
                        .font(xbold(30.0))
                        .text_color(WHITE)
                        .hint_text(egui::RichText::new("Title").font(xbold(30.0)).color(Color32::from_gray(0x44)))
                        .frame(egui::Frame::NONE)
                        .margin(egui::Margin::ZERO)
                        .desired_width(w),
                )
            })
            .inner;
        if e.focus_title {
            title_resp.request_focus();
            if e.is_new {
                let n = e.title.chars().count();
                set_cursor(&ctx, title_id, 0, n);
            }
            e.focus_title = false;
            e.focus_body = false;
        }
        if title_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Tab)) {
            title_enter = true;
        }
        ui.add_space(16.0);

        // Body
        let top = ui.cursor().min.y;
        let bg_idx = ui.painter().add(Shape::Noop);
        let mut decos: Vec<Deco> = Vec::new();
        let text_before = e.text.clone();
        let out = ui
            .scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(x0, top), vec2(w, f32::INFINITY))), |ui| {
                let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
                    let (job, d) = build(buf.as_str(), active, wrap);
                    decos = d;
                    ui.fonts_mut(|f| f.layout_job(job))
                };
                egui::TextEdit::multiline(&mut e.text)
                    .id(id)
                    .font(sans(16.0))
                    .text_color(TEXT2)
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::ZERO)
                    .desired_width(w)
                    .desired_rows(1)
                    .lock_focus(true)
                    .hint_text(egui::RichText::new("Start writing…  type / for formatting").font(sans(16.0)).color(Color32::from_gray(0x55)))
                    .layouter(&mut layouter)
                    .show(ui)
            })
            .inner;
        if e.focus_body {
            out.response.response.request_focus();
            e.focus_body = false;
        }
        if out.response.response.changed() || e.text != text_before {
            changed = true;
        }
        let g = out.galley.clone();
        let gp = out.galley_pos;
        let painter = ui.painter();
        let row_rect = |ci: usize| -> Rect { g.pos_from_cursor(CCursor::new(ci)).translate(gp.to_vec2()) };

        // Background: code blocks, then the active-line pill.
        let mut bg = Vec::new();
        for d in &decos {
            if let DecoKind::Code { first, last } = d.kind {
                let r0 = row_rect(d.at);
                let r1 = row_rect(d.end);
                let r = Rect::from_min_max(pos2(gp.x - 10.0, r0.min.y), pos2(gp.x + w + 10.0, r1.max.y));
                let rad = egui::CornerRadius { nw: if first { 12 } else { 0 }, ne: if first { 12 } else { 0 }, sw: if last { 12 } else { 0 }, se: if last { 12 } else { 0 } };
                bg.push(Shape::rect_filled(r, rad, CODE));
                let st = Stroke::new(2.0, GUIDE);
                bg.push(Shape::line_segment([pos2(r.min.x + 1.0, r.min.y + if first { 12.0 } else { 0.0 }), pos2(r.min.x + 1.0, r.max.y - if last { 12.0 } else { 0.0 })], st));
                bg.push(Shape::line_segment([pos2(r.max.x - 1.0, r.min.y + if first { 12.0 } else { 0.0 }), pos2(r.max.x - 1.0, r.max.y - if last { 12.0 } else { 0.0 })], st));
                if first {
                    bg.push(Shape::rect_stroke(Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + 24.0)), rad, st, StrokeKind::Inside));
                }
                if last {
                    bg.push(Shape::rect_stroke(Rect::from_min_max(pos2(r.min.x, r.max.y - 24.0), r.max), rad, st, StrokeKind::Inside));
                }
            }
        }
        if let Some(li) = active {
            let b = byte_at(&e.text, cursor.min(e.text.chars().count()));
            let (ls, le) = line_bounds(&e.text, b);
            let r0 = row_rect(char_at(&e.text, ls));
            let r1 = row_rect(char_at(&e.text, le));
            let ty = ctx.animate_value_with_time(id.with("pill-top"), r0.min.y - gp.y, 0.18);
            let th = ctx.animate_value_with_time(id.with("pill-h"), (r1.max.y - r0.min.y).max(LH), 0.18);
            let pill = Rect::from_min_size(pos2(gp.x - 14.0, gp.y + ty), vec2(w + 28.0, th));
            bg.push(Shape::rect_filled(pill, 14, PILL));
            let _ = li;
            cursor_screen = Some(row_rect(cursor));
        }
        painter.set(bg_idx, Shape::Vec(bg));

        // Foreground decorations replace the hidden markers.
        for d in &decos {
            let r = row_rect(d.at);
            let cy = r.min.y + LH / 2.0;
            match d.kind {
                DecoKind::Dot => {
                    painter.circle_filled(pos2(r.min.x + 3.0, cy), 2.6, WHITE);
                }
                DecoKind::Box(done) => {
                    let b = Rect::from_min_size(pos2(r.min.x, cy - 8.0), vec2(16.0, 16.0));
                    let hov = ui.rect_contains_pointer(b.expand(3.0));
                    if done {
                        painter.rect_filled(b, 4, WHITE);
                        painter.add(Shape::line(
                            vec![pos2(b.min.x + 3.5, b.center().y), pos2(b.min.x + 6.5, b.max.y - 4.0), pos2(b.max.x - 3.5, b.min.y + 4.5)],
                            Stroke::new(2.4, BLACK),
                        ));
                    } else {
                        painter.rect_stroke(b, 4, Stroke::new(2.0, if hov { WHITE } else { MUTED }), StrokeKind::Inside);
                    }
                    if hov {
                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    boxes.push((b, d.line));
                }
                DecoKind::Bar => {
                    let r1 = row_rect(d.end);
                    painter.rect_filled(Rect::from_min_max(pos2(r.min.x, r.min.y + 4.0), pos2(r.min.x + 3.0, r1.max.y - 4.0)), 2, WHITE);
                }
                DecoKind::Rule => {
                    painter.line_segment([pos2(gp.x, cy), pos2(gp.x + w, cy)], Stroke::new(2.0, BORDER));
                }
                DecoKind::Code { .. } => {}
            }
        }

        // Clicking below the text puts the cursor at the end.
        let tail = ui.allocate_response(vec2(avail, 240.0), Sense::click());
        if tail.clicked() {
            let n = e.text.chars().count();
            set_cursor(&ctx, id, n, n);
            out.response.response.request_focus();
        }
        if tail.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::Text);
        }
    });
    e.boxes = boxes;
    if title_enter {
        e.focus_body = true;
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
                if let Some((a, b)) = sel.or(e.last_sel) {
                    set_cursor(&ctx, id, a, b);
                }
            }
        }
    }

    // Open the slash menu after typing `/` at a line start or after a space.
    let post_state = egui::text_edit::TextEditState::load(&ctx, id);
    let post_cursor = post_state.as_ref().and_then(|s| s.cursor.char_range()).map(|r| r.primary.index.0).unwrap_or(cursor);
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
                e.palette = Some(Palette { slash: char_at(&e.text, sb), sel: 0, inserted: false });
            }
        }
    }
    // Close it when the cursor leaves the query, or the query stops matching.
    if let Some(p) = &mut e.palette {
        let q: Option<String> = if post_cursor > p.slash && e.text.chars().nth(p.slash) == Some('/') {
            let q: String = e.text.chars().skip(p.slash + 1).take(post_cursor - p.slash - 1).collect();
            if q.contains('\n') || q.starts_with('/') || q.starts_with(' ') {
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
                let list = matches(&q);
                if list.is_empty() {
                    e.palette = None;
                } else if changed {
                    p.sel = 0;
                }
            }
        }
    }

    // Floating toolbar.
    let (blk, isb, isi, iss) = styles_at(&e.text, post_cursor);
    let tools: Vec<(Result<Icon, &str>, Cmd, bool, String)> = vec![
        (Err("H"), Cmd::HeadingCycle, matches!(blk, Block::Heading(_)), "Heading".into()),
        (Err("B"), Cmd::Bold, isb, format!("Bold  {MOD} B")),
        (Err("I"), Cmd::Italic, isi, format!("Italic  {MOD} I")),
        (Err("S"), Cmd::Strike, iss, "Strikethrough".into()),
        (Ok(Icon::Bullets), Cmd::Bullet, blk == Block::Bullet, format!("Bullet list  {MOD} ⇧ 8")),
        (Ok(Icon::Numbers), Cmd::Number, blk == Block::Number, format!("Numbered list  {MOD} ⇧ 7")),
        (Ok(Icon::Checkbox), Cmd::Task, matches!(blk, Block::Task(_)), format!("Checkbox  {MOD} ⇧ 9")),
        (Ok(Icon::Quote), Cmd::Quote, blk == Block::Quote, "Quote".into()),
        (Ok(Icon::Code), Cmd::Code, false, "Inline code".into()),
        (Ok(Icon::Link), Cmd::Link, false, format!("Link  {MOD} K")),
    ];
    let slash_w = 96.0;
    let total = tools.len() as f32 * 42.0 + 3.0 * 12.0 + slash_w + 12.0;
    let bar = Rect::from_center_size(pos2(rect.center().x, rect.max.y - 18.0 - 26.0), vec2(total.min(rect.width() - 32.0), 52.0));
    let fg = ui.painter().clone().with_layer_id(egui::LayerId::new(egui::Order::Foreground, Id::new("toolbar"))).with_clip_rect(bar);
    fg.rect_filled(bar, 26, TOOLBAR);
    let mut x = bar.min.x + 6.0;
    for (i, (icon, cmd, on, tip)) in tools.into_iter().enumerate() {
        if i == 1 || i == 4 || i == 8 {
            fg.rect_filled(Rect::from_center_size(pos2(x + 6.0, bar.center().y), vec2(2.0, 22.0)), 1, GUIDE);
            x += 12.0;
        }
        let r = Rect::from_min_size(pos2(x, bar.min.y + 5.0), vec2(42.0, 42.0));
        let resp = ui.interact(r.intersect(bar), Id::new(("tool", i)), Sense::click()).on_hover_text(tip);
        if on {
            fg.circle_filled(r.center(), 21.0, WHITE);
        } else if resp.hovered() {
            fg.circle_filled(r.center(), 21.0, TOOLBAR_HOVER);
        }
        let col = if on { BLACK } else { WHITE };
        match icon {
            Ok(ic) => icons::paint(&fg, Rect::from_center_size(r.center(), vec2(21.0, 21.0)), ic, col),
            Err(t) => {
                let font = match t {
                    "I" => italic(17.0),
                    _ => xbold(17.0),
                };
                fg.text(r.center(), Align2::CENTER_CENTER, t, font, col);
                if t == "S" {
                    fg.line_segment([r.center() - vec2(7.0, 0.0), r.center() + vec2(7.0, 0.0)], Stroke::new(2.0, col));
                }
            }
        }
        if resp.clicked() {
            toolbar_cmd = Some(cmd);
        }
        x += 42.0;
    }
    fg.rect_filled(Rect::from_center_size(pos2(x + 6.0, bar.center().y), vec2(2.0, 22.0)), 1, GUIDE);
    x += 12.0;
    let r = Rect::from_min_size(pos2(x, bar.min.y + 5.0), vec2(slash_w, 42.0));
    let resp = ui.interact(r.intersect(bar), Id::new("tool-slash"), Sense::click()).on_hover_text(format!("Formatting menu  {MOD} /"));
    if resp.hovered() {
        fg.rect_filled(r, 21, TOOLBAR_HOVER);
    }
    fg.text(pos2(r.min.x + 14.0, r.center().y), Align2::LEFT_CENTER, "/", mono(16.0), WHITE);
    fg.text(pos2(r.min.x + 32.0, r.center().y), Align2::LEFT_CENTER, "Insert", semibold(12.0), MUTED);
    if resp.clicked() {
        open_palette_from_toolbar = true;
    }

    // Slash palette.
    if let (Some(p), Some(cr)) = (&e.palette, cursor_screen) {
        let q: String = e.text.chars().skip(p.slash + 1).take(post_cursor.saturating_sub(p.slash + 1)).collect();
        let list = matches(&q);
        let sel = p.sel.min(list.len().saturating_sub(1));
        let screen = ctx.content_rect();
        let wpal = 320.0;
        let h = 50.0 + (list.len().min(7) as f32) * 46.0 + 46.0;
        let mut pos = pos2(cr.min.x - 10.0, cr.max.y + 6.0);
        if pos.y + h > screen.max.y - 10.0 {
            pos.y = cr.min.y - 6.0 - h;
        }
        pos.x = pos.x.clamp(8.0, screen.max.x - wpal - 8.0);
        egui::Area::new(Id::new("palette")).order(egui::Order::Foreground).fixed_pos(pos).show(&ctx, |ui| {
            egui::Frame::new().fill(MENU).stroke(Stroke::new(LINE, BORDER)).corner_radius(18).inner_margin(6).show(ui, |ui| {
                ui.set_width(wpal - 12.0);
                let (r, _) = ui.allocate_exact_size(vec2(wpal - 12.0, 30.0), Sense::hover());
                ui.painter().text(pos2(r.min.x + 10.0, r.center().y), Align2::LEFT_CENTER, "F O R M A T", bold(10.0), MUTED);
                ui.painter().text(pos2(r.max.x - 10.0, r.center().y), Align2::RIGHT_CENTER, format!("/{q}"), mono(11.0), TEXT3);
                egui::ScrollArea::vertical().max_height(7.0 * 46.0).show(ui, |ui| {
                    for (i, c) in list.iter().enumerate() {
                        let (r, resp) = ui.allocate_exact_size(vec2(wpal - 12.0, 46.0), Sense::click());
                        let on = i == sel;
                        if on {
                            ui.painter().rect_filled(r, 12, WHITE);
                            if resp.rect.max.y > 0.0 {
                                ui.scroll_to_rect(r, None);
                            }
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, 12, MENU_HOVER);
                        }
                        let fgc = if on { BLACK } else { WHITE };
                        let ic = Rect::from_min_size(pos2(r.min.x + 10.0, r.center().y - 16.0), vec2(32.0, 32.0));
                        ui.painter().rect_stroke(ic, 9, Stroke::new(LINE, if on { Color32::from_gray(0xcc) } else { BORDER }), StrokeKind::Inside);
                        match c.icon {
                            Ok(icn) => icons::paint(ui.painter(), Rect::from_center_size(ic.center(), vec2(18.0, 18.0)), icn, fgc),
                            Err(t) => {
                                ui.painter().text(ic.center(), Align2::CENTER_CENTER, t, if t == "I" { italic(13.0) } else { xbold(13.0) }, fgc);
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
                let (r, _) = ui.allocate_exact_size(vec2(wpal - 12.0, 34.0), Sense::hover());
                ui.painter().line_segment([pos2(r.min.x, r.min.y + 4.0), pos2(r.max.x, r.min.y + 4.0)], Stroke::new(2.0, DIVIDER));
                ui.painter().text(pos2(r.min.x + 10.0, r.center().y + 3.0), Align2::LEFT_CENTER, "↑↓ or Tab to move · Enter to insert · Esc for a plain /", sans(11.0), MUTED);
            });
        });
    }

    // Toolbar and palette clicks act on the last selection and give focus back.
    let base = sel.or(e.last_sel).unwrap_or((post_cursor, post_cursor));
    if let Some(c) = pal_click {
        if let Some(p) = e.palette.take() {
            let (b0, b1) = (byte_at(&e.text, p.slash), byte_at(&e.text, post_cursor.max(p.slash + 1)));
            e.text.replace_range(b0..b1, "");
            let (a, b) = apply(&mut e.text, (p.slash, p.slash), c, true);
            set_cursor(&ctx, id, a, b);
            ctx.memory_mut(|m| m.request_focus(id));
            changed = true;
        }
    }
    if let Some(c) = toolbar_cmd {
        let (a, b) = apply(&mut e.text, base, c, false);
        set_cursor(&ctx, id, a, b);
        ctx.memory_mut(|m| m.request_focus(id));
        changed = true;
    }
    if open_palette_from_toolbar && e.palette.is_none() {
        let at = base.1;
        let b = byte_at(&e.text, at);
        let needs_space = b > 0 && !matches!(e.text[..b].chars().last(), Some(' ') | Some('\n'));
        let ins = if needs_space { " /" } else { "/" };
        e.text.insert_str(b, ins);
        let slash = at + ins.chars().count() - 1;
        e.palette = Some(Palette { slash, sel: 0, inserted: true });
        set_cursor(&ctx, id, slash + 1, slash + 1);
        ctx.memory_mut(|m| m.request_focus(id));
        changed = true;
    }

    if !deferred.is_empty() {
        e.pending = deferred;
        ctx.request_repaint();
    }
    if changed {
        if e.dirty_since.is_none() {
            e.dirty_since = Some(now);
        } else {
            e.dirty_since = Some(now);
        }
        ctx.request_repaint();
    }
    let _ = (&app.actions, Action::Read);
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
        assert_eq!(run("", (0, 0), Cmd::Link), ("[text](url)".into(), (1, 5)));
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
