//! Reading view: rendered markdown with foldable headings and tickable checkboxes.
use crate::app::{Action, App};
use crate::icons::{self, Icon};
use crate::md::{self, Block, Style};
use crate::theme::*;
use crate::widgets;
use egui::{pos2, text::LayoutJob, vec2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, TextFormat, Ui};
use std::collections::HashSet;
use std::ops::Range;
use std::path::PathBuf;

pub const COL_MAX: f32 = 760.0;

/// Column inside `avail` width: (left x offset, width).
pub fn column(avail: f32) -> (f32, f32) {
    let (pl, pr) = if avail < 720.0 { (40.0, 20.0) } else { (56.0, 40.0) };
    let w = (avail - pl - pr).min(COL_MAX - 96.0);
    let left = ((avail - w) / 2.0).max(pl);
    (left, w)
}

pub struct Inline {
    pub job: LayoutJob,
    /// (char range, url) for clickable links.
    pub links: Vec<(Range<usize>, String)>,
}

pub fn format_for(st: Style, base: &FontId, color: Color32, line_height: f32) -> TextFormat {
    let size = base.size;
    let is_bold_base = base.family != egui::FontFamily::Proportional && base.family != egui::FontFamily::Name("italic".into());
    let font = if st.code {
        mono(size * 0.82)
    } else if st.bold && !is_bold_base {
        bold(size)
    } else if st.italic && !st.bold && !is_bold_base {
        italic(size)
    } else {
        base.clone()
    };
    let mut f = TextFormat { font_id: font, color, line_height: Some(line_height), ..Default::default() };
    if st.italic && (st.bold || is_bold_base) {
        f.italics = true;
    }
    if st.bold {
        f.color = WHITE;
    }
    if st.strike {
        f.strikethrough = Stroke::new(1.5, MUTED);
        f.color = MUTED;
    }
    if st.code {
        f.background = GUIDE;
        f.color = WHITE;
        f.line_height = Some((size * 1.35).round());
        f.valign = egui::Align::Center;
        f.expand_bg = 1.0;
    }
    if st.link {
        f.underline = Stroke::new(1.0, WHITE);
        f.color = WHITE;
    }
    f
}

/// Rendered (markers hidden) inline text.
pub fn inline(s: &str, range: Range<usize>, font: FontId, color: Color32, line_height: f32, wrap: f32) -> Inline {
    let mut job = LayoutJob::default();
    let spans = md::inline(s, range);
    let mut links = Vec::new();
    let mut chars = 0;
    for (i, sp) in spans.iter().enumerate() {
        if sp.style.marker {
            if sp.style.link && s[sp.range.clone()].starts_with("](") {
                // Link text runs back to the opening `[`.
                let url = md::link_url(s, sp.range.clone()).unwrap_or_default();
                let mut start = chars;
                for prev in spans[..i].iter().rev() {
                    if prev.style.marker && &s[prev.range.clone()] == "[" {
                        break;
                    }
                    if !prev.style.marker {
                        start -= s[prev.range.clone()].chars().count();
                    }
                }
                links.push((start..chars, url));
            }
            continue;
        }
        let t = &s[sp.range.clone()];
        chars += t.chars().count();
        job.append(t, 0.0, format_for(sp.style, &font, color, line_height));
    }
    job.wrap.max_width = wrap;
    Inline { job, links }
}

/// Paint inline text and open links on click.
fn paint_inline(ui: &mut Ui, x: f32, inl: Inline) -> egui::Response {
    let galley = widgets::layout(ui, inl.job);
    let top = ui.cursor().min.y;
    let rect = Rect::from_min_size(pos2(x, top), galley.size());
    let row = ui.allocate_rect(Rect::from_min_size(pos2(ui.max_rect().min.x, top), vec2(ui.max_rect().width(), galley.size().y)), Sense::hover());
    // Only the text and what follows it react, so markers to its left keep their own clicks.
    let resp = ui.interact(Rect::from_min_max(pos2(x, top), pos2(row.rect.max.x, row.rect.max.y)), row.id.with("t"), Sense::click());
    let hit = |pos: egui::Pos2| -> Option<String> {
        let c = galley.cursor_from_pos(pos - rect.min);
        inl.links.iter().find(|(r, _)| r.contains(&c.index.0)).map(|(_, u)| u.clone())
    };
    if let Some(p) = resp.hover_pos() {
        if rect.contains(p) && hit(p).is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    if resp.clicked() {
        if let Some(url) = resp.interact_pointer_pos().filter(|p| rect.contains(*p)).and_then(hit) {
            if url.starts_with("http") || url.starts_with("mailto:") {
                let _ = opener::open(url);
            }
        }
    }
    ui.painter().galley(rect.min, galley, TEXT2);
    resp
}

fn heading_font(level: u8) -> (FontId, f32) {
    match level {
        1 => (xbold(24.0), 30.0),
        2 => (xbold(20.0), 27.0),
        3 => (bold(18.0), 25.0),
        _ => (bold(16.0), 24.0),
    }
}

/* ---------- blocks ---------- */

struct Li {
    line: usize,
    block: Block,
    marker: String,
    text: String,
    ind: usize,
    kids: Vec<Li>,
}

enum Blk {
    P { line: usize, text: String },
    Quote { line: usize, text: String },
    Code { line: usize, text: String },
    Rule,
    List { items: Vec<Li> },
    H { line: usize, level: u8, raw: String, text: String },
}

enum Node {
    Leaf(Blk),
    Sec { line: usize, level: u8, raw: String, text: String, kids: Vec<Node> },
}

fn indent_width(l: &str, c: &md::Line) -> usize {
    l[..c.indent].chars().map(|ch| if ch == '\t' { 2 } else { 1 }).sum()
}

fn parse_blocks(lines: &[&str]) -> Vec<Blk> {
    let mut out = Vec::new();
    let mut i = 0;
    let is_list = |b: Block| matches!(b, Block::Bullet | Block::Number | Block::Task(_));
    while i < lines.len() {
        let l = lines[i];
        let c = md::classify(l);
        if l.trim().is_empty() {
            i += 1;
            continue;
        }
        match c.block {
            Block::Fence => {
                let s = i;
                i += 1;
                let mut body = Vec::new();
                while i < lines.len() && md::classify(lines[i]).block != Block::Fence {
                    body.push(lines[i]);
                    i += 1;
                }
                i += 1;
                out.push(Blk::Code { line: s, text: body.join("\n") });
            }
            Block::Heading(level) => {
                out.push(Blk::H { line: i, level, raw: l.trim().to_string(), text: l[c.content.clone()].to_string() });
                i += 1;
            }
            Block::Rule => {
                out.push(Blk::Rule);
                i += 1;
            }
            Block::Quote => {
                let s = i;
                let mut q = Vec::new();
                while i < lines.len() && md::classify(lines[i]).block == Block::Quote {
                    let cc = md::classify(lines[i]);
                    q.push(lines[i][cc.content].to_string());
                    i += 1;
                }
                out.push(Blk::Quote { line: s, text: q.join(" ") });
            }
            b if is_list(b) => {
                // Build the nesting from indentation, like the Electron reader.
                let mut flat: Vec<Li> = Vec::new();
                while i < lines.len() {
                    let ci = md::classify(lines[i]);
                    if !is_list(ci.block) {
                        break;
                    }
                    flat.push(Li {
                        line: i,
                        block: ci.block,
                        marker: lines[i][ci.marker.clone()].trim().to_string(),
                        text: lines[i][ci.content.clone()].to_string(),
                        ind: indent_width(lines[i], &ci),
                        kids: vec![],
                    });
                    i += 1;
                }
                out.push(Blk::List { items: nest(flat) });
            }
            _ => {
                let s = i;
                let mut p = Vec::new();
                while i < lines.len() {
                    let cc = md::classify(lines[i]);
                    if cc.block != Block::Plain || lines[i].trim().is_empty() {
                        break;
                    }
                    p.push(lines[i].trim().to_string());
                    i += 1;
                }
                out.push(Blk::P { line: s, text: p.join(" ") });
            }
        }
    }
    out
}

/// Turn a flat run of list items into a tree: an item owns the deeper-indented items after it.
fn nest(flat: Vec<Li>) -> Vec<Li> {
    fn take(items: &mut std::iter::Peekable<std::vec::IntoIter<Li>>, parent_ind: Option<usize>) -> Vec<Li> {
        let mut out = Vec::new();
        while let Some(next) = items.peek() {
            if let Some(p) = parent_ind {
                if next.ind <= p {
                    break;
                }
            }
            let mut it = items.next().unwrap();
            it.kids = take(items, Some(it.ind));
            out.push(it);
        }
        out
    }
    take(&mut flat.into_iter().peekable(), None)
}

fn tree(blocks: Vec<Blk>) -> Vec<Node> {
    fn parse(it: &mut std::iter::Peekable<std::vec::IntoIter<Blk>>, min: u8) -> Vec<Node> {
        let mut out = Vec::new();
        while let Some(b) = it.peek() {
            if let Blk::H { level, .. } = b {
                if *level <= min {
                    return out;
                }
                let Some(Blk::H { line, level, raw, text }) = it.next() else { unreachable!() };
                let kids = parse(it, level);
                out.push(Node::Sec { line, level, raw, text, kids });
            } else {
                out.push(Node::Leaf(it.next().unwrap()));
            }
        }
        out
    }
    parse(&mut blocks.into_iter().peekable(), 0)
}

fn count(li: &Li) -> usize {
    li.kids.iter().map(|k| 1 + count(k)).sum()
}

pub fn words(text: &str) -> usize {
    text.chars().map(|c| if "#>*_`~-[]()".contains(c) { ' ' } else { c }).collect::<String>().split_whitespace().count()
}

/* ---------- rendering ---------- */

struct R {
    path: PathBuf,
    x0: f32,
    w: f32,
    avail: f32,
    folds: HashSet<String>,
    lfolds: HashSet<usize>,
    toggle_fold: Option<String>,
    toggle_list: Option<usize>,
    toggle_task: Option<usize>,
    edit_line: Option<usize>,
}

/// Content that slides open and closed: drawn in a child, clipped to `t` of its height.
fn reveal(ui: &mut Ui, id: egui::Id, open: bool, dur: f32, add: impl FnOnce(&mut Ui)) {
    let t = ui.ctx().animate_bool_with_time(id, open, dur);
    if t <= 0.0 {
        return;
    }
    let top = ui.cursor().min;
    let avail = ui.available_width();
    let hid = id.with("h");
    let prev_h: f32 = ui.ctx().data(|d| d.get_temp(hid)).unwrap_or(f32::INFINITY);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(top, vec2(avail, f32::INFINITY))).layout(egui::Layout::top_down(egui::Align::Min)));
    if t < 1.0 {
        let visible = Rect::from_min_size(pos2(ui.clip_rect().min.x, top.y), vec2(ui.clip_rect().width(), prev_h * t));
        child.set_clip_rect(visible.intersect(ui.clip_rect()));
        child.multiply_opacity(t);
    }
    add(&mut child);
    let full = child.min_rect().height();
    ui.ctx().data_mut(|d| d.insert_temp(hid, full));
    ui.allocate_exact_size(vec2(avail, full * t), Sense::hover());
}

fn nodes(ui: &mut Ui, list: &[Node], r: &mut R, top_level: bool) {
    for (i, n) in list.iter().enumerate() {
        if i > 0 {
            ui.add_space(14.0);
        }
        match n {
            Node::Leaf(b) => leaf(ui, b, r),
            Node::Sec { line, level, raw, text, kids } => {
                if *level >= 2 && !(top_level && i == 0) {
                    ui.add_space(8.0);
                }
                let (font, lh) = heading_font(*level);
                let color = if *level >= 4 { TEXT2 } else { WHITE };
                let foldable = !kids.is_empty();
                let closed = foldable && r.folds.contains(raw);
                let top = ui.cursor().min.y;
                let resp = paint_inline(ui, r.x0, inline(text, 0..text.len(), font, color, lh, r.w));
                if foldable {
                    let cr = Rect::from_center_size(pos2(r.x0 - 34.0 + 14.0, top + lh / 2.0), vec2(28.0, 28.0));
                    let cid = ui.id().with(("chev", raw));
                    let chev = ui.interact(cr, cid, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    let h = ui.ctx().animate_bool_with_time(cid.with("h"), chev.hovered(), 0.12);
                    if h > 0.0 {
                        ui.painter().circle_filled(cr.center(), 14.0, PILL.gamma_multiply(h));
                    }
                    let rot = ui.ctx().animate_bool_with_time_and_easing(cid.with("r"), !closed, 0.25, egui::emath::easing::back_out);
                    let col = if closed || chev.hovered() { WHITE } else { MUTED };
                    icons::paint_rot(ui.painter(), Rect::from_center_size(cr.center(), vec2(14.0, 14.0)), Icon::Chevron, col, rot * std::f32::consts::FRAC_PI_2);
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if (resp.clicked() && !resp.double_clicked()) || chev.clicked() {
                        r.toggle_fold = Some(raw.clone());
                    }
                }
                if resp.double_clicked() {
                    r.edit_line = Some(*line);
                }
                if foldable {
                    let id = ui.id().with(("sec", raw));
                    reveal(ui, id, !closed, 0.22, |ui| {
                        ui.add_space(14.0);
                        nodes(ui, kids, r, false);
                    });
                }
            }
        }
    }
}

fn leaf(ui: &mut Ui, b: &Blk, r: &mut R) {
    let (x0, w, avail) = (r.x0, r.w, r.avail);
    match b {
        Blk::P { line, text } => {
            let resp = paint_inline(ui, x0, inline(text, 0..text.len(), sans(16.0), TEXT2, 26.0, w));
            if resp.double_clicked() {
                r.edit_line = Some(*line);
            }
        }
        Blk::Quote { line, text } => {
            let top = ui.cursor().min.y;
            ui.add_space(2.0);
            let resp = paint_inline(ui, x0 + 17.0, inline(text, 0..text.len(), italic(16.0), TEXT3, 26.0, w - 17.0));
            ui.add_space(2.0);
            let bottom = ui.cursor().min.y;
            ui.painter().rect_filled(Rect::from_min_max(pos2(x0, top), pos2(x0 + 3.0, bottom)), 2, WHITE);
            if resp.double_clicked() {
                r.edit_line = Some(*line);
            }
        }
        Blk::Code { line, text } => {
            let mut job = widgets::simple_job(text, mono(13.0), TEXT2, f32::INFINITY, 0);
            job.sections[0].format.line_height = Some(20.0);
            let g = widgets::layout(ui, job);
            let h = g.size().y.max(20.0) + 28.0;
            let (rr, resp) = ui.allocate_exact_size(vec2(avail, h), Sense::click());
            let box_ = Rect::from_min_size(pos2(x0, rr.min.y), vec2(w, h));
            ui.painter().rect_filled(box_, 14, CODE);
            ui.painter().rect_stroke(box_, 14, Stroke::new(LINE, GUIDE), StrokeKind::Inside);
            ui.painter().with_clip_rect(box_.shrink(2.0).intersect(ui.clip_rect())).galley(box_.min + vec2(14.0, 14.0), g, TEXT2);
            if resp.double_clicked() {
                r.edit_line = Some(*line);
            }
        }
        Blk::Rule => {
            let (rr, _) = ui.allocate_exact_size(vec2(avail, 10.0), Sense::hover());
            ui.painter().line_segment([pos2(x0, rr.center().y), pos2(x0 + w, rr.center().y)], Stroke::new(2.0, BORDER));
        }
        Blk::List { items } => {
            let g = items.iter().any(|k| !k.kids.is_empty());
            for (i, it) in items.iter().enumerate() {
                if i > 0 {
                    ui.add_space(4.0);
                }
                list_item(ui, it, g, x0, r);
            }
        }
        Blk::H { .. } => {}
    }
}

fn list_item(ui: &mut Ui, it: &Li, g: bool, x: f32, r: &mut R) {
    let has = !it.kids.is_empty();
    let closed = has && r.lfolds.contains(&it.line);
    let top = ui.cursor().min.y;
    let mut mx = x;
    let id = ui.id().with(("li", it.line));
    let mut chev_resp = None;
    if g {
        mx -= 10.0;
        let cr = Rect::from_min_size(pos2(mx, top), vec2(28.0, 28.0));
        if has {
            let resp = ui.interact(cr, id.with("lc"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let h = ui.ctx().animate_bool_with_time(id.with("lch"), resp.hovered(), 0.12);
            if h > 0.0 {
                ui.painter().circle_filled(cr.center(), 14.0, PILL.gamma_multiply(h));
            }
            let rot = ui.ctx().animate_bool_with_time_and_easing(id.with("lcr"), !closed, 0.25, egui::emath::easing::back_out);
            icons::paint_rot(ui.painter(), Rect::from_center_size(cr.center(), vec2(14.0, 14.0)), Icon::Chevron, if closed { WHITE } else { MUTED }, rot * std::f32::consts::FRAC_PI_2);
            chev_resp = Some(resp);
        }
        mx += 28.0;
    }
    let mut done = false;
    let mut box_resp = None;
    let marker_w = match it.block {
        Block::Bullet => {
            ui.painter().circle_filled(pos2(mx + 2.5, top + 14.0), 2.5, WHITE);
            15.0
        }
        Block::Number => {
            let g = ui.painter().layout_no_wrap(it.marker.clone(), sans(15.0), MUTED);
            let gw = g.size().x;
            ui.painter().galley(pos2(mx, top + 14.0 - g.size().y / 2.0), g, MUTED);
            gw + 8.0
        }
        Block::Task(d) => {
            done = d;
            let b = Rect::from_min_size(pos2(mx, top + 6.0), vec2(16.0, 16.0));
            let resp = ui.interact(b.expand(4.0), id.with("box"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let t = ui.ctx().animate_bool_with_time(id.with("boxt"), d, 0.12);
            ui.painter().rect_filled(b, 4, WHITE.gamma_multiply(t));
            let border = if resp.hovered() { WHITE } else { widgets::lerp_color(MUTED, WHITE, t) };
            ui.painter().rect_stroke(b, 4, Stroke::new(2.0, border), StrokeKind::Inside);
            if d {
                icons::paint(ui.painter(), b.shrink(2.0), Icon::Check, BLACK);
            }
            box_resp = Some(resp);
            26.0
        }
        _ => 0.0,
    };
    let color = if done { MUTED } else { TEXT2 };
    let tx = mx + marker_w;
    let tw = (r.x0 + r.w - tx).max(40.0);
    let mut inl = inline(&it.text, 0..it.text.len(), sans(16.0), color, 22.0, tw);
    if done {
        for s in &mut inl.job.sections {
            s.format.strikethrough = Stroke::new(1.5, MUTED);
            s.format.color = MUTED;
        }
    }
    ui.add_space(3.0);
    let galley_end = paint_inline_end(ui, tx, inl);
    let resp = galley_end.0;
    if has && closed {
        let n = count(it);
        let label = format!("{n} {}", if n == 1 { "item" } else { "items" });
        let gl = ui.painter().layout_no_wrap(label, sans(11.0), MUTED);
        let end = galley_end.1;
        let br = Rect::from_min_size(pos2(end.x + 10.0, end.y - 9.0), vec2(gl.size().x + 16.0, 18.0));
        ui.painter().rect_stroke(br, 10, Stroke::new(LINE, BORDER), StrokeKind::Inside);
        ui.painter().galley(pos2(br.min.x + 8.0, br.center().y - gl.size().y / 2.0), gl, MUTED);
    }
    ui.add_space(3.0);
    if let Some(b) = box_resp {
        if b.clicked() {
            r.toggle_task = Some(it.line);
        }
    }
    if let Some(c) = chev_resp {
        if c.clicked() {
            r.toggle_list = Some(it.line);
        }
    }
    if resp.double_clicked() {
        r.edit_line = Some(it.line);
    }
    if has {
        let kid_x = x + 3.0 + 2.0 + 18.0;
        let kg = it.kids.iter().any(|k| !k.kids.is_empty());
        reveal(ui, id.with("kids"), !closed, 0.2, |ui| {
            ui.add_space(4.0);
            let start = ui.cursor().min.y;
            for (i, k) in it.kids.iter().enumerate() {
                if i > 0 {
                    ui.add_space(6.0);
                }
                list_item(ui, k, kg, kid_x, r);
            }
            let end = ui.cursor().min.y;
            ui.painter().rect_filled(Rect::from_min_max(pos2(x + 3.0, start), pos2(x + 5.0, end)), 0, GUIDE);
        });
    }
}

/// Like `paint_inline`, also returning where the text's last row ends (right, vertical centre).
fn paint_inline_end(ui: &mut Ui, x: f32, inl: Inline) -> (egui::Response, egui::Pos2) {
    let galley = widgets::layout(ui, inl.job.clone());
    let top = ui.cursor().min.y;
    let end = galley.rows.last().map(|row| {
        let rr = row.rect().translate(vec2(x, top));
        pos2(rr.max.x, rr.center().y)
    });
    let resp = paint_inline(ui, x, inl);
    (resp, end.unwrap_or(pos2(x, top + 11.0)))
}

pub fn show(app: &mut App, ui: &mut Ui, path: PathBuf) {
    let Some(item) = app.tree.find(&path).cloned() else { return };
    let text = item.text.clone();
    let trimmed = text.trim_end_matches('\n');
    let lines: Vec<&str> = trimmed.split('\n').collect();
    let tree = tree(parse_blocks(&lines));
    let rect = ui.max_rect();
    let mut r = R {
        path: path.clone(),
        x0: 0.0,
        w: 0.0,
        avail: 0.0,
        folds: app.folds.get(&path).cloned().unwrap_or_default(),
        lfolds: app.list_folds.get(&path).cloned().unwrap_or_default(),
        toggle_fold: None,
        toggle_list: None,
        toggle_task: None,
        edit_line: None,
    };

    egui::ScrollArea::vertical().id_salt(("reader", &path)).auto_shrink([false, false]).show(ui, |ui| {
        let avail = ui.available_width();
        let (left, w) = column(avail);
        r.x0 = ui.max_rect().min.x + left;
        r.w = w;
        r.avail = avail;
        ui.add_space(8.0);
        if !crate::editor::starts_with_h1(&text) {
            let mut tj = widgets::simple_job(&item.name, xbold(30.0), WHITE, w, 0);
            tj.sections[0].format.line_height = Some(36.0);
            let tg = widgets::layout(ui, tj);
            let (tr, _) = ui.allocate_exact_size(vec2(avail, tg.size().y), Sense::hover());
            ui.painter().galley(pos2(r.x0, tr.min.y), tg, WHITE);
            ui.add_space(16.0);
        }
        if text.trim().is_empty() {
            let (rr, _) = ui.allocate_exact_size(vec2(avail, 20.0), Sense::hover());
            ui.painter().text(pos2(r.x0, rr.min.y), egui::Align2::LEFT_TOP, "Empty note. Press E or the pencil to start writing.", sans(13.0), MUTED);
        } else {
            nodes(ui, &tree, &mut r, true);
        }
        // Footer
        ui.add_space(22.0);
        let (fr, _) = ui.allocate_exact_size(vec2(avail, 30.0), Sense::hover());
        ui.painter().line_segment([pos2(r.x0, fr.min.y + 1.0), pos2(r.x0 + w, fr.min.y + 1.0)], Stroke::new(2.0, DIVIDER));
        let n = words(&text);
        ui.painter().text(
            pos2(r.x0, fr.min.y + 12.0),
            egui::Align2::LEFT_TOP,
            format!("{n} {} · edited {}", if n == 1 { "word" } else { "words" }, crate::store::ago(item.mtime)),
            sans(11.0),
            MUTED,
        );
        ui.add_space(140.0);
    });

    // Floating pencil.
    if fab(ui, rect, Icon::Edit, "Start editing", "E").clicked() {
        app.actions.push(Action::EditNote(path.clone(), None));
    }

    if let Some(k) = r.toggle_fold {
        let f = app.folds.entry(path.clone()).or_default();
        if !f.remove(&k) {
            f.insert(k);
        }
        crate::store::save_folds(&app.folds);
    }
    if let Some(l) = r.toggle_list {
        let f = app.list_folds.entry(path.clone()).or_default();
        if !f.remove(&l) {
            f.insert(l);
        }
    }
    if let Some(i) = r.toggle_task {
        let mut out: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
        if let Some(l) = out.get_mut(i) {
            let c = md::classify(l);
            let at = c.marker.start + 3;
            if let Block::Task(done) = c.block {
                l.replace_range(at..at + 1, if done { " " } else { "x" });
            }
        }
        app.actions.push(Action::WriteNote(path.clone(), out.join("\n")));
    }
    if let Some(l) = r.edit_line {
        app.actions.push(Action::EditNote(path, Some(l)));
    }
    let _ = &r.path;
}

/// The round white button in the bottom-right corner.
pub fn fab(ui: &mut Ui, rect: Rect, icon: Icon, tip: &str, key: &str) -> egui::Response {
    let c = pos2(rect.max.x - 26.0 - 30.0, rect.max.y - 26.0 - 30.0);
    let fr = Rect::from_center_size(c, vec2(60.0, 60.0));
    let layer = egui::LayerId::new(egui::Order::Middle, ui.id().with("fab"));
    let mut fui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(fr));
    let id = ui.id().with(("fab", tip));
    let resp = fui.interact(fr, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    widgets::tip(&fui, &resp, tip, key, true);
    let target = if resp.is_pointer_button_down_on() { 0.94 } else if resp.hovered() { 1.06 } else { 1.0 };
    let scale = ui.ctx().animate_value_with_time(id.with("s"), target, 0.18);
    let p = fui.painter();
    p.circle_filled(c, 30.0 * scale, WHITE);
    icons::paint(p, Rect::from_center_size(c, vec2(26.0, 26.0) * scale), icon, BLACK);
    let _ = &mut fui;
    resp
}
