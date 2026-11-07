//! The home/folder grid: Keep-style masonry of note and folder cards, pinned on top.
use crate::app::{mi, sep, Action, App, Drag, MenuAt};
use crate::icons::{self, Icon};
use crate::md::{self, Block};
use crate::store::Item;
use crate::theme::*;
use crate::widgets;
use egui::{pos2, text::LayoutJob, vec2, Align2, Color32, Rect, Sense, Stroke, StrokeKind, TextFormat, Ui};
use std::path::PathBuf;
use std::sync::Arc;

const GAP: f32 = 10.0;
const MARGIN: f32 = 22.0;
const PAD: f32 = 14.0;

/// One laid-out row of a card preview.
enum Row {
    Text { galley: Arc<egui::Galley>, indent: f32 },
    Task { galley: Arc<egui::Galley>, indent: f32, done: bool },
    Bullet { galley: Arc<egui::Galley>, indent: f32 },
    Rule,
}

struct Card {
    title: Arc<egui::Galley>,
    rows: Vec<Row>,
    meta: Option<Arc<egui::Galley>>,
    height: f32,
}

fn inline_job(s: &str, range: std::ops::Range<usize>, size: f32, color: Color32, heading: bool, done: bool) -> LayoutJob {
    let mut job = LayoutJob::default();
    for sp in md::inline(s, range) {
        if sp.style.marker {
            continue;
        }
        let st = sp.style;
        let font = if st.code {
            mono(size - 1.0)
        } else if heading || st.bold {
            bold(size)
        } else if st.italic {
            italic(size)
        } else {
            sans(size)
        };
        let mut f = TextFormat { font_id: font, color, line_height: Some(18.0), ..Default::default() };
        if st.strike || done {
            f.strikethrough = Stroke::new(1.0, MUTED);
            f.color = MUTED;
        }
        if st.code {
            f.background = GUIDE;
            f.color = WHITE;
        }
        if st.italic && (heading || st.bold) {
            f.italics = true;
        }
        job.append(&s[sp.range], 0.0, f);
    }
    job
}

fn build_card(ui: &Ui, item: &Item, w: f32, all: &Item) -> Card {
    let inner = w - PAD * 2.0;
    let title_w = inner - if item.is_folder() { 24.0 } else { 0.0 } - if item.pinned { 20.0 } else { 0.0 };
    let mut tj = widgets::simple_job(&item.name, xbold(15.0), WHITE, title_w.max(20.0), 3);
    tj.sections[0].format.line_height = Some(20.0);
    tj.wrap.overflow_character = Some('…');
    let title = widgets::layout(ui, tj);
    let mut rows = Vec::new();
    let mut meta = None;
    if item.is_folder() {
        for k in item.children.iter().take(3) {
            let label = format!("{}{}", k.name, if k.is_folder() { "/" } else { ".md" });
            let mut j = widgets::simple_job(&label, sans(12.0), TEXT3, inner, 1);
            j.wrap.overflow_character = Some('…');
            j.sections[0].format.line_height = Some(20.0);
            rows.push(Row::Text { galley: widgets::layout(ui, j), indent: 0.0 });
        }
        let n = item.children.len();
        let label = if n == 1 { "1 item".to_string() } else { format!("{n} items") };
        meta = Some(widgets::layout(ui, widgets::simple_job(&label, sans(11.0), MUTED, inner, 1)));
    } else {
        let mut lines: Vec<&str> = item.text.lines().map(|l| l.trim_end()).collect();
        while lines.first().map(|l| l.trim().is_empty()).unwrap_or(false) {
            lines.remove(0);
        }
        if lines.first().map(|l| l.starts_with("# ")).unwrap_or(false) {
            lines.remove(0);
        }
        let mut in_fence = false;
        let lines: Vec<&str> = lines
            .into_iter()
            .filter(|l| {
                if l.trim_start().starts_with("```") {
                    in_fence = !in_fence;
                    return false;
                }
                !l.trim().is_empty() && !in_fence
            })
            .take(8)
            .collect();
        for l in lines {
            let c = md::classify(l);
            let indent = (c.level(l) as f32 * 10.0).min(40.0);
            match c.block {
                Block::Rule => rows.push(Row::Rule),
                Block::Task(done) => {
                    let mut j = inline_job(l, c.content.clone(), 12.0, TEXT3, false, done);
                    j.wrap.max_width = inner - indent - 19.0;
                    j.wrap.max_rows = 1;
                    j.wrap.overflow_character = Some('…');
                    rows.push(Row::Task { galley: widgets::layout(ui, j), indent, done });
                }
                Block::Bullet => {
                    let mut j = inline_job(l, c.content.clone(), 12.0, TEXT3, false, false);
                    j.wrap.max_width = inner - indent - 12.0;
                    j.wrap.max_rows = 1;
                    j.wrap.overflow_character = Some('…');
                    rows.push(Row::Bullet { galley: widgets::layout(ui, j), indent });
                }
                Block::Heading(_) => {
                    let mut j = inline_job(l, c.content.clone(), 12.0, WHITE, true, false);
                    j.wrap.max_width = inner;
                    j.wrap.max_rows = 2;
                    rows.push(Row::Text { galley: widgets::layout(ui, j), indent: 0.0 });
                }
                Block::Number => {
                    let mut j = LayoutJob::default();
                    j.append(
                        &format!("{} ", l[c.marker.clone()].trim()),
                        0.0,
                        TextFormat { font_id: sans(12.0), color: MUTED, line_height: Some(18.0), ..Default::default() },
                    );
                    let rest = inline_job(l, c.content.clone(), 12.0, TEXT3, false, false);
                    for s in rest.sections {
                        j.append(&rest.text[s.byte_range.start.0..s.byte_range.end.0], 0.0, s.format);
                    }
                    j.wrap.max_width = inner - indent;
                    j.wrap.max_rows = 1;
                    j.wrap.overflow_character = Some('…');
                    rows.push(Row::Text { galley: widgets::layout(ui, j), indent });
                }
                _ => {
                    let range = if c.block == Block::Quote { c.content.clone() } else { 0..l.len() };
                    let mut j = inline_job(l, range, 12.0, TEXT3, false, false);
                    j.wrap.max_width = inner;
                    j.wrap.max_rows = 1;
                    j.wrap.overflow_character = Some('…');
                    rows.push(Row::Text { galley: widgets::layout(ui, j), indent: 0.0 });
                }
            }
        }
    }
    let _ = all;
    let mut h = PAD + title.size().y.max(20.0);
    if !rows.is_empty() {
        h += if item.is_folder() { 8.0 } else { 6.0 };
        for (i, r) in rows.iter().enumerate() {
            if i > 0 {
                h += 3.0;
            }
            h += match r {
                Row::Rule => 10.0,
                Row::Text { galley, .. } | Row::Task { galley, .. } | Row::Bullet { galley, .. } => galley.size().y.max(18.0),
            };
        }
    }
    if let Some(m) = &meta {
        h += 8.0 + m.size().y;
    }
    h += PAD;
    Card { title, rows, meta, height: h }
}

fn paint_card(p: &egui::Painter, rect: Rect, item: &Item, card: &Card, hover: f32, selected: bool, lifted: bool) {
    let fill = if lifted { SURFACE } else { BLACK };
    p.rect_filled(rect, 22, fill);
    let (w, col) = if selected || lifted { (2.5, WHITE) } else { (LINE, widgets::lerp_color(BORDER, BORDER_HOVER, hover)) };
    p.rect_stroke(rect, 22, Stroke::new(w, col), StrokeKind::Inside);
    let x0 = rect.min.x + PAD;
    let mut y = rect.min.y + PAD;
    let mut tx = x0;
    if item.is_folder() {
        icons::paint(p, Rect::from_min_size(pos2(x0, y + 2.0), vec2(16.0, 16.0)), Icon::FolderFill, WHITE);
        tx += 24.0;
    }
    p.galley(pos2(tx, y), card.title.clone(), WHITE);
    if item.pinned {
        icons::paint(p, Rect::from_min_size(pos2(rect.max.x - PAD - 14.0, y + 3.0), vec2(14.0, 14.0)), Icon::PinFill, WHITE);
    }
    y += card.title.size().y.max(20.0);
    if !card.rows.is_empty() {
        y += if item.is_folder() { 8.0 } else { 6.0 };
    }
    for (i, r) in card.rows.iter().enumerate() {
        if i > 0 {
            y += 3.0;
        }
        match r {
            Row::Rule => {
                p.line_segment([pos2(x0, y + 5.0), pos2(rect.max.x - PAD, y + 5.0)], Stroke::new(2.0, BORDER));
                y += 10.0;
            }
            Row::Text { galley, indent } => {
                p.galley(pos2(x0 + indent, y), galley.clone(), TEXT3);
                y += galley.size().y.max(18.0);
            }
            Row::Bullet { galley, indent } => {
                p.text(pos2(x0 + indent, y + 9.0), Align2::LEFT_CENTER, "•", sans(12.0), MUTED);
                p.galley(pos2(x0 + indent + 12.0, y), galley.clone(), TEXT3);
                y += galley.size().y.max(18.0);
            }
            Row::Task { galley, indent, done } => {
                let b = Rect::from_min_size(pos2(x0 + indent, y + 3.5), vec2(11.0, 11.0));
                if *done {
                    p.rect_filled(b, 3, WHITE);
                } else {
                    p.rect_stroke(b, 3, Stroke::new(2.0, MUTED), StrokeKind::Inside);
                }
                p.galley(pos2(x0 + indent + 19.0, y), galley.clone(), TEXT3);
                y += galley.size().y.max(18.0);
            }
        }
    }
    if let Some(m) = &card.meta {
        y += 8.0;
        p.galley(pos2(x0, y), m.clone(), MUTED);
    }
}

pub fn show(app: &mut App, ui: &mut Ui, dir: PathBuf) {
    let Some(folder) = app.tree.find(&dir).cloned() else { return };
    let rect = ui.max_rect();
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);

    if folder.children.is_empty() {
        let mut job = widgets::simple_job(&format!("Nothing here yet.\nPress + or {}+N to write a note.", widgets::MOD), sans(13.0), MUTED, 400.0, 0);
        job.sections[0].format.line_height = Some(21.0);
        job.halign = egui::Align::Center;
        let g = widgets::layout(ui, job);
        ui.painter().galley(rect.center() - vec2(0.0, g.size().y / 2.0 + 20.0), g, MUTED);
    }

    // Groups, with the dragged card's group in its live order.
    let mut pinned: Vec<&Item> = folder.children.iter().filter(|c| c.pinned).collect();
    let mut rest: Vec<&Item> = folder.children.iter().filter(|c| !c.pinned).collect();
    if let Some(d) = &app.drag {
        let reorder = |v: &mut Vec<&Item>| {
            let mut out: Vec<&Item> = d.order.iter().filter_map(|p| v.iter().find(|c| &c.path == p).copied()).collect();
            for c in v.iter() {
                if !out.iter().any(|o| o.path == c.path) {
                    out.push(c)
                }
            }
            *v = out;
        };
        if d.pinned {
            reorder(&mut pinned)
        } else {
            reorder(&mut rest)
        }
    }

    let mut new_rects: Vec<(PathBuf, Rect)> = Vec::new();
    let mut dragged_card: Option<(Item, Card, Rect)> = None;
    let selected = app.selected.clone();
    let tree = app.tree.clone();
    let pointer = ctx.input(|i| i.pointer.hover_pos());

    egui::ScrollArea::vertical().id_salt(("grid", &dir)).auto_shrink([false, false]).show(ui, |ui| {
        let width = ui.available_width() - MARGIN * 2.0;
        let cols = ((width + GAP) / (230.0 + GAP)).floor().max(2.0) as usize;
        let cw = (width - GAP * (cols as f32 - 1.0)) / cols as f32;
        let origin = ui.min_rect().min + vec2(MARGIN, 4.0);
        let mut y0 = 0.0;
        // Card slots relative to the grid origin.
        let place = |ui: &mut Ui, list: &[&Item], y0: f32, out: &mut Vec<(Item, Card, Rect)>| -> f32 {
            let mut h = vec![y0; cols];
            for item in list {
                let card = build_card(ui, item, cw, &tree);
                let c = (0..cols).min_by(|&a, &b| h[a].partial_cmp(&h[b]).unwrap()).unwrap();
                let r = Rect::from_min_size(egui::Pos2::ZERO + vec2(c as f32 * (cw + GAP), h[c]), vec2(cw, card.height));
                h[c] += card.height + GAP;
                out.push(((*item).clone(), card, r));
            }
            h.into_iter().fold(y0, f32::max)
        };
        let mut placed = Vec::new();
        if !pinned.is_empty() {
            let mut lj = LayoutJob::default();
            lj.append("PINNED", 0.0, TextFormat { font_id: bold(11.0), color: MUTED, extra_letter_spacing: 1.0, ..Default::default() });
            let lg = widgets::layout(ui, lj);
            ui.painter().galley(origin + vec2(6.0, 0.0), lg, MUTED);
            y0 = place(ui, &pinned, 24.0, &mut placed);
            ui.painter().line_segment([origin + vec2(4.0, y0 + 4.0), origin + vec2(width - 4.0, y0 + 4.0)], Stroke::new(2.0, DIVIDER));
            y0 += 18.0;
        }
        let end = place(ui, &rest, y0, &mut placed);
        ui.allocate_rect(Rect::from_min_size(ui.min_rect().min, vec2(ui.available_width(), end + 4.0 + 160.0)), Sense::hover());

        // Cards glide to their slots when the order or the column count changes.
        let mut dragged_slot = None;
        for (item, card, slot) in placed {
            let id = ui.id().with(("card", &item.path));
            let rel = widgets::anim_vec2(&ctx, id.with("pos"), slot.min.to_vec2(), 0.28);
            let r = Rect::from_min_size(origin + rel, slot.size());
            let resp = ui.interact(r, id, Sense::click_and_drag());
            new_rects.push((item.path.clone(), Rect::from_min_size(origin + slot.min.to_vec2(), slot.size())));
            let is_dragged = app.drag.as_ref().map(|d| d.path == item.path).unwrap_or(false);
            if is_dragged {
                dragged_slot = Some(r);
                dragged_card = Some((item.clone(), card, r));
            } else {
                let hover = ctx.animate_bool_with_time(id.with("h"), resp.hovered() && app.drag.is_none(), 0.12);
                let press = ctx.animate_bool_with_time(id.with("press"), resp.is_pointer_button_down_on() && app.drag.is_none(), 0.1);
                let pr = Rect::from_center_size(r.center(), r.size() * (1.0 - 0.03 * press));
                paint_card(ui.painter(), pr, &item, &card, hover, selected.contains(&item.path), false);
            }
            let mods = ui.input(|i| i.modifiers);
            if resp.clicked() {
                if mods.command || !app.selected.is_empty() {
                    if !app.selected.remove(&item.path) {
                        app.selected.insert(item.path.clone());
                    }
                } else if item.is_folder() {
                    app.actions.push(Action::OpenFolder(item.path.clone()));
                } else {
                    app.actions.push(Action::OpenNote(item.path.clone()));
                }
            }
            if resp.secondary_clicked() {
                let targets: Vec<PathBuf> = if app.selected.contains(&item.path) { app.selected.iter().cloned().collect() } else { vec![item.path.clone()] };
                let pos = ui.input(|i| i.pointer.interact_pos()).unwrap_or(r.center());
                let n = targets.len();
                let all_pinned = targets.iter().all(|t| tree.find(t).map(|i| i.pinned).unwrap_or(false));
                let mut items = Vec::new();
                if n == 1 {
                    items.push(mi(Icon::Note, "Open", "Enter", if item.is_folder() { Action::OpenFolder(item.path.clone()) } else { Action::OpenNote(item.path.clone()) }));
                }
                items.push(mi(if all_pinned { Icon::Pin } else { Icon::PinFill }, if all_pinned { "Unpin" } else { "Pin" }, "", Action::Pin(targets.clone(), !all_pinned)));
                if n == 1 {
                    items.push(mi(Icon::Rename, "Rename", "F2", Action::Rename(item.path.clone())));
                }
                items.push(mi(Icon::Move, "Move to…", "", Action::Move(targets.clone())));
                items.push(sep());
                let label = if n == 1 { "Delete".to_string() } else { format!("Delete {n} items") };
                items.push(mi(Icon::Trash, &label, "Del", Action::Delete(targets, false)));
                app.open_menu(ui, MenuAt::Point(pos), items);
            }
            if resp.drag_started() && app.selected.is_empty() {
                let grab = ui.input(|i| i.pointer.press_origin()).map(|p| p - r.min).unwrap_or_default();
                let group = if item.pinned { &pinned } else { &rest };
                app.drag = Some(Drag { path: item.path.clone(), grab, pinned: item.pinned, order: group.iter().map(|c| c.path.clone()).collect(), started: now });
            }
        }
        // Dashed outline where the dragged card will land.
        if let Some(r) = dragged_slot {
            ui.painter().extend(egui::Shape::dashed_line(&widgets::rounded_path(r.shrink(1.0), 21.0), Stroke::new(2.0, Color32::from_gray(0x6a)), 7.0, 5.0));
        }
        // Scroll when dragging near the top or bottom edge.
        if app.drag.is_some() {
            if let Some(p) = pointer {
                if p.y < rect.min.y + 60.0 {
                    ui.scroll_with_delta(vec2(0.0, 12.0));
                    ctx.request_repaint();
                } else if p.y > rect.max.y - 60.0 {
                    ui.scroll_with_delta(vec2(0.0, -12.0));
                    ctx.request_repaint();
                }
            }
        }
    });

    // Dragging: the card follows the pointer, lifted, and its group reorders live.
    if let Some(d) = &mut app.drag {
        let released = ctx.input(|i| !i.pointer.primary_down());
        if let (Some(ptr), Some((item, card, slot))) = (pointer, &dragged_card) {
            if let Some((target, _)) = new_rects.iter().find(|(p, r)| r.contains(ptr) && p != &d.path && d.order.contains(p)) {
                let from = d.order.iter().position(|p| p == &d.path).unwrap();
                let to = d.order.iter().position(|p| p == target).unwrap();
                let moved = d.order.remove(from);
                d.order.insert(to, moved);
            }
            let r = Rect::from_min_size(ptr - d.grab, slot.size());
            let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("dragcard"));
            let lift = widgets::ease_pop(widgets::progress(&ctx, d.started, 0.18));
            let s = 1.0 + 0.05 * lift;
            let c = r.center().to_vec2();
            ctx.set_transform_layer(layer, egui::emath::TSTransform::from_translation(c) * egui::emath::TSTransform::from_scaling(s) * egui::emath::TSTransform::from_translation(-c));
            let painter = ctx.layer_painter(layer);
            paint_card(&painter, r, item, card, 0.0, false, true);
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        if released {
            let d = app.drag.take().unwrap();
            let orig_group: Vec<PathBuf> = folder.children.iter().filter(|c| c.pinned == d.pinned).map(|c| c.path.clone()).collect();
            if orig_group != d.order {
                let name = |p: &PathBuf| crate::store::file_name(p);
                let (pinned_names, order_names) = if d.pinned {
                    (d.order.iter().map(name).collect(), folder.children.iter().filter(|c| !c.pinned).map(|c| c.file.clone()).collect())
                } else {
                    (folder.children.iter().filter(|c| c.pinned).map(|c| c.file.clone()).collect(), d.order.iter().map(name).collect())
                };
                app.actions.push(Action::Reorder { dir: dir.clone(), pinned: pinned_names, order: order_names });
            }
        }
    }
    app.card_rects = new_rects;

    // Floating add button, or the selection bar.
    let sel_id = ui.id().with("selbar-at");
    if app.selected.is_empty() {
        ctx.data_mut(|d| d.remove::<f64>(sel_id));
        let fab = crate::reader::fab(ui, rect, Icon::Plus, "New note or folder", &format!("{} N", widgets::MOD));
        if fab.clicked() {
            let items = vec![
                mi(Icon::Note, "New note", &format!("{} N", widgets::MOD), Action::NewNote(dir.clone())),
                mi(Icon::Folder, "New folder", &format!("{} ⇧ N", widgets::MOD), Action::NewFolder(dir.clone())),
                mi(Icon::Import, "Import .md files", "", Action::Import(dir.clone())),
            ];
            app.open_menu(ui, MenuAt::Above(fab.rect), items);
        }
    } else {
        let shown_at = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(sel_id, || now));
        let t = widgets::progress(&ctx, shown_at, 0.26);
        let e = widgets::ease_pop(t);
        let sel: Vec<PathBuf> = app.selected.iter().cloned().collect();
        let n = sel.len();
        let all_pinned = sel.iter().all(|t| app.tree.find(t).map(|i| i.pinned).unwrap_or(false));
        let mut buttons: Vec<(Icon, &str, Action)> = vec![
            (if all_pinned { Icon::Pin } else { Icon::PinFill }, if all_pinned { "Unpin" } else { "Pin" }, Action::Pin(sel.clone(), !all_pinned)),
            (Icon::Move, "Move", Action::Move(sel.clone())),
        ];
        if n == 1 {
            buttons.push((Icon::Rename, "Rename", Action::Rename(sel[0].clone())));
        }
        buttons.push((Icon::Trash, "Delete", Action::Delete(sel.clone(), false)));
        let widths: Vec<f32> = buttons.iter().map(|b| 16.0 + 18.0 + 8.0 + ui.fonts_mut(|f| f.layout_no_wrap(b.1.into(), bold(13.0), WHITE).size().x) + 16.0).collect();
        let total = widths.iter().sum::<f32>() + 2.0 * (widths.len() as f32 - 1.0) + 10.0;
        let bar = Rect::from_center_size(pos2(rect.center().x, rect.max.y - 24.0 - 28.0 + (1.0 - e) * 30.0), vec2(total, 56.0));
        let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::new("selbar"));
        let mut sui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(bar));
        sui.multiply_opacity(t);
        let fg = sui.painter().clone();
        fg.rect_filled(bar, 28, TOOLBAR);
        let mut x = bar.min.x + 5.0;
        for ((icon, text, act), w) in buttons.into_iter().zip(widths) {
            let r = Rect::from_min_size(pos2(x, bar.min.y + 5.0), vec2(w, 46.0));
            let bid = egui::Id::new(("selbtn", text));
            let resp = sui.interact(r, bid, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let h = ctx.animate_bool_with_time(bid.with("h"), resp.hovered(), 0.12);
            if h > 0.0 {
                fg.rect_filled(r, 23, TOOLBAR_HOVER.gamma_multiply(h));
            }
            icons::paint(&fg, Rect::from_min_size(pos2(r.min.x + 16.0, r.center().y - 9.0), vec2(18.0, 18.0)), icon, WHITE);
            fg.text(pos2(r.min.x + 42.0, r.center().y), Align2::LEFT_CENTER, text, bold(13.0), WHITE);
            if resp.clicked() {
                app.actions.push(act);
            }
            x += w + 2.0;
        }
    }
}
