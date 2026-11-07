//! Small custom-drawn controls shared by every screen.
use crate::icons::{self, Icon};
use crate::theme::*;
use egui::{text::LayoutJob, vec2, Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui};

/// Round 44px icon button.
pub fn ibtn(ui: &mut Ui, rect: Rect, icon: Icon, tip: &str, dim: bool) -> Response {
    let id = ui.id().with(("ibtn", tip, rect.min.x as i32, rect.min.y as i32));
    let r = ui.interact(rect, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let r = if tip.is_empty() { r } else { r.on_hover_text(tip) };
    let p = ui.painter();
    let pressed = r.is_pointer_button_down_on();
    if r.hovered() {
        p.circle_filled(rect.center(), rect.width() / 2.0 * if pressed { 0.94 } else { 1.0 }, PILL);
    }
    let col = if dim && !r.hovered() { MUTED } else { WHITE };
    icons::paint(p, Rect::from_center_size(rect.center(), vec2(22.0, 22.0)), icon, col);
    r
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
    let p = ui.painter();
    let rad = CornerRadius::same((rect.height() / 2.0) as u8);
    let fg = if primary {
        p.rect_filled(rect, rad, if r.hovered() { Color32::from_gray(0xe6) } else { WHITE });
        BLACK
    } else {
        p.rect_stroke(rect, rad, Stroke::new(LINE, if r.hovered() { Color32::from_gray(0x55) } else { BORDER }), StrokeKind::Inside);
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
