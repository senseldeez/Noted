//! Small custom-drawn controls shared by every screen.
use crate::icons::{self, Icon};
use crate::theme::*;
use egui::{text::LayoutJob, vec2, Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui};

/// Round 44px icon button with a hover pill, a press squeeze and a tooltip.
pub fn ibtn(ui: &mut Ui, rect: Rect, icon: Icon, tip_text: &str, key: &str, dim: bool) -> Response {
    ibtn_sized(ui, rect, icon, 22.0, tip_text, key, dim)
}

pub fn ibtn_sized(ui: &mut Ui, rect: Rect, icon: Icon, size: f32, tip_text: &str, key: &str, dim: bool) -> Response {
    let id = ui.id().with(("ibtn", tip_text, rect.min.x as i32, rect.min.y as i32));
    let r = ui.interact(rect, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    tip(ui, &r, tip_text, key, false);
    let h = ui.ctx().animate_bool_with_time(id.with("h"), r.hovered(), 0.12);
    let s = ui.ctx().animate_bool_with_time(id.with("p"), r.is_pointer_button_down_on(), 0.12);
    let scale = 1.0 - 0.06 * s;
    let p = ui.painter();
    if h > 0.0 {
        p.circle_filled(rect.center(), rect.width() / 2.0 * scale, PILL.gamma_multiply(h));
    }
    let col = if dim { MUTED } else { WHITE };
    icons::paint(p, Rect::from_center_size(rect.center(), vec2(size, size) * scale), icon, col);
    r
}

/// Ease-out used for most transitions (cubic-bezier(.2,.8,.2,1)).
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Springy ease with a small overshoot, for things that pop in (cubic-bezier(.2,1.2,.4,1)).
pub fn ease_pop(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let c1 = 1.2;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// Progress 0..1 of an animation that started at `t0` and lasts `dur`; keeps repainting until done.
pub fn progress(ctx: &egui::Context, t0: f64, dur: f32) -> f32 {
    let now = ctx.input(|i| i.time);
    let t = ((now - t0) as f32 / dur).clamp(0.0, 1.0);
    if t < 1.0 {
        ctx.request_repaint();
    }
    t
}

/// White pill tooltip with an optional key hint, shown after a short hover.
/// `above` puts it over the element (toolbar, floating button) instead of below.
pub fn tip(ui: &Ui, resp: &Response, label: &str, key: &str, above: bool) {
    if label.is_empty() {
        return;
    }
    let ctx = ui.ctx();
    let id = resp.id.with("tip");
    let now = ctx.input(|i| i.time);
    let overlay: bool = ctx.data(|d| d.get_temp(egui::Id::new("overlay-open"))).unwrap_or(false);
    if !resp.hovered() || ctx.dragged_id().is_some() {
        ctx.data_mut(|d| d.remove::<f64>(id));
        return;
    }
    // Pressing the element hides its tip until the pointer leaves it.
    if resp.is_pointer_button_down_on() || resp.clicked() || overlay {
        ctx.data_mut(|d| d.insert_temp(id, f64::INFINITY));
        return;
    }
    let start = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(id, || now));
    if !start.is_finite() {
        return;
    }
    let el = (now - start) as f32;
    if el < 0.38 {
        ctx.request_repaint_after(std::time::Duration::from_secs_f32(0.38 - el + 0.005));
        return;
    }
    let a = ((el - 0.38) / 0.1).min(1.0);
    if a < 1.0 {
        ctx.request_repaint();
    }
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("tip")));
    let mut job = LayoutJob::default();
    job.append(label, 0.0, egui::TextFormat { font_id: bold(12.0), color: BLACK, ..Default::default() });
    if !key.is_empty() {
        job.append(key, 6.0, egui::TextFormat { font_id: mono(12.0), color: Color32::from_gray(0x55), ..Default::default() });
    }
    let g = painter.layout_job(job);
    let size = g.size() + vec2(18.0, 10.0);
    let screen = ctx.content_rect();
    let r = resp.rect;
    let x = (r.center().x - size.x / 2.0).clamp(screen.min.x + 8.0, (screen.max.x - size.x - 8.0).max(screen.min.x + 8.0));
    let y = if above { r.min.y - size.y - 8.0 } else { r.max.y + 8.0 };
    let rect = Rect::from_min_size(egui::pos2(x, y), size);
    painter.rect_filled(rect, 8, WHITE.gamma_multiply(a));
    painter.galley(rect.min + vec2(9.0, 5.0), g, BLACK.gamma_multiply(a));
}

/// Width a pill button needs for `label` with an optional icon.
pub fn pill_width(ui: &Ui, label: &str, icon: bool) -> f32 {
    let w = ui.fonts_mut(|f| f.layout_no_wrap(label.into(), bold(13.0), WHITE).size().x);
    w + 36.0 + if icon { 25.0 } else { 0.0 }
}

/// 40px rounded button; `primary` is white on black, otherwise outlined.
pub fn pillbtn(ui: &mut Ui, rect: Rect, label: &str, icon: Option<Icon>, primary: bool) -> Response {
    let id = ui.id().with(("pill", label, rect.min.x as i32, rect.min.y as i32));
    let r = ui.interact(rect, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let h = ui.ctx().animate_bool_with_time(id.with("h"), r.hovered(), 0.12);
    let s = ui.ctx().animate_bool_with_time(id.with("p"), r.is_pointer_button_down_on(), 0.12);
    let rect = Rect::from_center_size(rect.center(), rect.size() * (1.0 - 0.04 * s));
    let p = ui.painter();
    let rad = CornerRadius::same((rect.height() / 2.0) as u8);
    let fg = if primary {
        p.rect_filled(rect, rad, lerp_color(WHITE, Color32::from_gray(0xe6), h));
        BLACK
    } else {
        p.rect_stroke(rect, rad, Stroke::new(LINE, lerp_color(BORDER, Color32::from_gray(0x55), h)), StrokeKind::Inside);
        WHITE
    };
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(label.into(), bold(13.0), fg));
    let total = galley.size().x + if icon.is_some() { 25.0 } else { 0.0 };
    let mut x = rect.center().x - total / 2.0;
    if let Some(ic) = icon {
        icons::paint(p, Rect::from_min_size(egui::pos2(x, rect.center().y - 9.0), vec2(18.0, 18.0)), ic, fg);
        x += 25.0;
    }
    p.galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, fg);
    r
}

/// Keyboard hint chip like `Ctrl F`.
pub fn kbd(ui: &Ui, right_center: egui::Pos2, text: &str) -> f32 {
    let g = ui.fonts_mut(|f| f.layout_no_wrap(text.into(), mono(11.0), TEXT2));
    let w = g.size().x + 12.0;
    let r = Rect::from_min_size(egui::pos2(right_center.x - w, right_center.y - 9.0), vec2(w, 18.0));
    ui.painter().rect_stroke(r, 6, Stroke::new(LINE, BORDER), StrokeKind::Inside);
    ui.painter().galley(egui::pos2(r.min.x + 6.0, r.center().y - g.size().y / 2.0), g, TEXT2);
    w
}

pub fn text(ui: &Ui, pos: egui::Pos2, align: Align2, s: &str, font: FontId, color: Color32) -> Rect {
    ui.painter().text(pos, align, s, font, color)
}

/// Lay out `job` and return its galley.
pub fn layout(ui: &Ui, job: LayoutJob) -> std::sync::Arc<egui::Galley> {
    ui.fonts_mut(|f| f.layout_job(job))
}

pub fn simple_job(s: &str, font: FontId, color: Color32, wrap: f32, max_rows: usize) -> LayoutJob {
    let mut job = LayoutJob::simple(s.to_string(), font, color, wrap);
    job.wrap.max_rows = if max_rows == 0 { usize::MAX } else { max_rows };
    job.wrap.break_anywhere = false;
    job
}

pub const MOD: &str = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" };

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// Eased move towards `target` (cubic-bezier(.2,.8,.2,1) over `dur`), restarting when the target changes.
pub fn anim_vec2(ctx: &egui::Context, id: egui::Id, target: egui::Vec2, dur: f32) -> egui::Vec2 {
    let now = ctx.input(|i| i.time);
    // (from, to, start)
    let st: Option<(egui::Vec2, egui::Vec2, f64)> = ctx.data(|d| d.get_temp(id));
    let (from, to, t0) = match st {
        None => {
            ctx.data_mut(|d| d.insert_temp(id, (target, target, now)));
            return target;
        }
        Some(s) => s,
    };
    let cur = |t0: f64, from: egui::Vec2, to: egui::Vec2| {
        let t = ((now - t0) as f32 / dur).clamp(0.0, 1.0);
        from + (to - from) * ease_out(t)
    };
    if (to - target).length() > 0.5 {
        let pos = cur(t0, from, to);
        ctx.data_mut(|d| d.insert_temp(id, (pos, target, now)));
        ctx.request_repaint();
        return pos;
    }
    let t = ((now - t0) as f32 / dur).clamp(0.0, 1.0);
    if t < 1.0 {
        ctx.request_repaint();
    }
    cur(t0, from, to)
}

/// Points around a rounded rectangle, for dashed outlines.
pub fn rounded_path(r: Rect, rad: f32) -> Vec<egui::Pos2> {
    let mut pts = Vec::new();
    let corners = [
        (egui::pos2(r.max.x - rad, r.min.y + rad), -90.0_f32),
        (egui::pos2(r.max.x - rad, r.max.y - rad), 0.0),
        (egui::pos2(r.min.x + rad, r.max.y - rad), 90.0),
        (egui::pos2(r.min.x + rad, r.min.y + rad), 180.0),
    ];
    for (c, a0) in corners {
        for k in 0..=6 {
            let a = (a0 + 90.0 * k as f32 / 6.0).to_radians();
            pts.push(egui::pos2(c.x + rad * a.cos(), c.y + rad * a.sin()));
        }
    }
    pts.push(pts[0]);
    pts
}
