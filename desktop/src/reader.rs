//! Reading view: rendered markdown with foldable headings and tickable checkboxes.
use crate::app::{Action, App};
use crate::icons::{self, Icon};
use crate::md::{self, Block, Style};
use crate::theme::*;
use crate::widgets;
use egui::{pos2, text::LayoutJob, vec2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, TextFormat, Ui};
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
    let resp = ui.allocate_rect(Rect::from_min_size(pos2(ui.max_rect().min.x, top), vec2(ui.max_rect().width(), galley.size().y)), Sense::click());
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

pub fn show(app: &mut App, ui: &mut Ui, path: PathBuf) {
    let Some(item) = app.tree.find(&path).cloned() else { return };
    let text = item.text.clone();
    let lines: Vec<&str> = text.lines().collect();
    let folds = app.folds.entry(path.clone()).or_default();
    let mut toggle_fold = None;
    let mut toggle_task: Option<usize> = None;
    let mut edit = false;

    egui::ScrollArea::vertical().id_salt(("reader", &path)).auto_shrink([false, false]).show(ui, |ui| {
        let avail = ui.available_width();
        let (left, w) = column(avail);
        let x0 = ui.max_rect().min.x + left;
        ui.add_space(8.0);
        // Title
        let mut tj = widgets::simple_job(&item.name, xbold(30.0), WHITE, w, 0);
        tj.sections[0].format.line_height = Some(36.0);
        let tg = widgets::layout(ui, tj);
        let (tr, tresp) = ui.allocate_exact_size(vec2(avail, tg.size().y), Sense::click());
        ui.painter().galley(pos2(x0, tr.min.y), tg, WHITE);
        if tresp.double_clicked() {
            edit = true;
        }
        ui.add_space(16.0);

        let mut i = 0;
        let mut first = true;
        let mut hidden_until_level: Option<u8> = None;
        let gap = |ui: &mut Ui, first: &mut bool, g: f32| {
            if !*first {
                ui.add_space(g)
            }
            *first = false;
        };
        let mut in_list = false;
        while i < lines.len() {
            let l = lines[i];
            let c = md::classify(l);
            // Folding: skip until a heading at the same or a higher level.
            if let Some(lv) = hidden_until_level {
                if let Block::Heading(h) = c.block {
                    if h <= lv {
                        hidden_until_level = None;
                    }
                }
                if hidden_until_level.is_some() {
                    i += 1;
                    continue;
                }
            }
            if l.trim().is_empty() {
                in_list = false;
                i += 1;
                continue;
            }
            match c.block {
                Block::Heading(level) => {
                    in_list = false;
                    gap(ui, &mut first, if level >= 2 { 22.0 } else { 14.0 });
                    let (font, lh) = heading_font(level);
                    let color = if level >= 4 { TEXT2 } else { WHITE };
                    // Does this section have content?
                    let has_body = lines[i + 1..]
                        .iter()
                        .take_while(|x| !matches!(md::classify(x).block, Block::Heading(h) if h <= level))
                        .any(|x| !x.trim().is_empty());
                    let closed = folds.contains(&i);
                    let inl = inline(l, c.content.clone(), font, color, lh, w);
                    let top = ui.cursor().min.y;
                    let resp = paint_inline(ui, x0, inl);
                    if has_body {
                        let cr = Rect::from_center_size(pos2(x0 - 34.0 + 14.0, top + lh / 2.0), vec2(28.0, 28.0));
                        let hov = ui.rect_contains_pointer(cr) || resp.hovered();
                        if ui.rect_contains_pointer(cr) {
                            ui.painter().circle_filled(cr.center(), 14.0, PILL);
                        }
                        let col = if closed || hov { WHITE } else { MUTED };
                        icons::paint_rot(ui.painter(), Rect::from_center_size(cr.center(), vec2(14.0, 14.0)), Icon::Chevron, col, if closed { 0.0 } else { std::f32::consts::FRAC_PI_2 });
                        if resp.clicked() || ui.interact(cr, ui.id().with(("chev", i)), Sense::click()).clicked() {
                            toggle_fold = Some(i);
                        }
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }
                    if closed {
                        hidden_until_level = Some(level);
                    }
                    i += 1;
                }
                Block::Fence => {
                    in_list = false;
                    gap(ui, &mut first, 14.0);
                    let mut code = Vec::new();
                    i += 1;
                    while i < lines.len() && md::classify(lines[i]).block != Block::Fence {
                        code.push(lines[i]);
                        i += 1;
                    }
                    i += 1;
                    let body = code.join("\n");
                    let mut job = widgets::simple_job(&body, mono(13.0), TEXT2, f32::INFINITY, 0);
                    job.sections[0].format.line_height = Some(20.0);
                    let g = widgets::layout(ui, job);
                    let h = g.size().y + 28.0;
                    let (r, _) = ui.allocate_exact_size(vec2(avail, h), Sense::hover());
                    let box_ = Rect::from_min_size(pos2(x0, r.min.y), vec2(w, h));
                    ui.painter().rect_filled(box_, 14, CODE);
                    ui.painter().rect_stroke(box_, 14, Stroke::new(LINE, GUIDE), StrokeKind::Inside);
                    ui.painter().with_clip_rect(box_.shrink(2.0)).galley(box_.min + vec2(14.0, 14.0), g, TEXT2);
                }
                Block::Rule => {
                    in_list = false;
                    gap(ui, &mut first, 14.0);
                    let (r, _) = ui.allocate_exact_size(vec2(avail, 10.0), Sense::hover());
                    ui.painter().line_segment([pos2(x0, r.center().y), pos2(x0 + w, r.center().y)], Stroke::new(2.0, BORDER));
                    i += 1;
                }
                Block::Quote => {
                    in_list = false;
                    gap(ui, &mut first, 14.0);
                    let mut parts = Vec::new();
                    while i < lines.len() && md::classify(lines[i]).block == Block::Quote {
                        let cc = md::classify(lines[i]);
                        parts.push(lines[i][cc.content].to_string());
                        i += 1;
                    }
                    let joined = parts.join(" ");
                    let inl = inline(&joined, 0..joined.len(), italic(16.0), TEXT3, 26.0, w - 17.0);
                    let top = ui.cursor().min.y;
                    paint_inline(ui, x0 + 17.0, inl);
                    let bottom = ui.cursor().min.y;
                    ui.painter().rect_filled(Rect::from_min_max(pos2(x0, top + 2.0), pos2(x0 + 3.0, bottom - 2.0)), 2, WHITE);
                }
                Block::Bullet | Block::Number | Block::Task(_) => {
                    gap(ui, &mut first, if in_list { 4.0 } else { 14.0 });
                    in_list = true;
                    let indent = c.level(l) as f32 * 28.0;
                    let x = x0 + indent;
                    let top = ui.cursor().min.y;
                    let (marker_w, done) = match c.block {
                        Block::Bullet => {
                            ui.painter().circle_filled(pos2(x + 2.5, top + 14.0), 2.6, WHITE);
                            (15.0, false)
                        }
                        Block::Number => {
                            let n = l[c.marker.clone()].trim().to_string();
                            let g = ui.painter().layout_no_wrap(n, sans(15.0), MUTED);
                            let gw = g.size().x;
                            ui.painter().galley(pos2(x, top + 14.0 - g.size().y / 2.0), g, MUTED);
                            (gw.max(14.0) + 8.0, false)
                        }
                        Block::Task(done) => {
                            let b = Rect::from_min_size(pos2(x, top + 6.0), vec2(16.0, 16.0));
                            let resp = ui.interact(b.expand(4.0), ui.id().with(("task", i)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                            if done {
                                ui.painter().rect_filled(b, 4, WHITE);
                                ui.painter().add(egui::Shape::line(
                                    vec![pos2(b.min.x + 3.5, b.center().y), pos2(b.min.x + 6.5, b.max.y - 4.0), pos2(b.max.x - 3.5, b.min.y + 4.5)],
                                    Stroke::new(2.4, BLACK),
                                ));
                            } else {
                                ui.painter().rect_stroke(b, 4, Stroke::new(2.0, if resp.hovered() { WHITE } else { MUTED }), StrokeKind::Inside);
                            }
                            if resp.clicked() {
                                toggle_task = Some(i);
                            }
                            (26.0, done)
                        }
                        _ => (0.0, false),
                    };
                    let color = if done { MUTED } else { TEXT2 };
                    let mut inl = inline(l, c.content.clone(), sans(16.0), color, 22.0, w - indent - marker_w);
                    if done {
                        for s in &mut inl.job.sections {
                            s.format.strikethrough = Stroke::new(1.5, MUTED);
                            s.format.color = MUTED;
                        }
                    }
                    ui.add_space(3.0);
                    paint_inline(ui, x + marker_w, inl);
                    ui.add_space(3.0);
                    i += 1;
                }
                Block::Plain => {
                    in_list = false;
                    gap(ui, &mut first, 14.0);
                    let mut parts = Vec::new();
                    while i < lines.len() {
                        let cc = md::classify(lines[i]);
                        if cc.block != Block::Plain || lines[i].trim().is_empty() {
                            break;
                        }
                        parts.push(lines[i].trim().to_string());
                        i += 1;
                    }
                    let joined = parts.join("\n");
                    let inl = inline(&joined, 0..joined.len(), sans(16.0), TEXT2, 26.0, w);
                    paint_inline(ui, x0, inl);
                }
            }
        }
        if lines.iter().all(|l| l.trim().is_empty()) {
            let (r, resp) = ui.allocate_exact_size(vec2(avail, 60.0), Sense::click());
            ui.painter().text(pos2(x0, r.min.y + 10.0), egui::Align2::LEFT_TOP, "Empty note. Press E to start writing.", sans(15.0), MUTED);
            if resp.clicked() {
                edit = true;
            }
        }
        // Footer
        ui.add_space(22.0);
        let (r, _) = ui.allocate_exact_size(vec2(avail, 30.0), Sense::hover());
        ui.painter().line_segment([pos2(x0, r.min.y), pos2(x0 + w, r.min.y)], Stroke::new(2.0, DIVIDER));
        let words = text.split_whitespace().count();
        ui.painter().text(
            pos2(x0, r.min.y + 12.0),
            egui::Align2::LEFT_TOP,
            format!("{} words · {}", words, crate::store::file_name(&path)),
            sans(11.0),
            MUTED,
        );
        ui.add_space(140.0);
    });

    if let Some(i) = toggle_fold {
        let f = app.folds.entry(path.clone()).or_default();
        if !f.remove(&i) {
            f.insert(i);
        }
    }
    if let Some(i) = toggle_task {
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
    if edit {
        app.actions.push(Action::EditNote(path));
    }
}
