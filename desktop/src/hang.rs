//! Hanging indents for the editor: when a list item, quote or indented line wraps, its
//! continuation rows line up with the text after the marker instead of the left edge.
//! egui has no per-paragraph indent, so each line is laid out on its own and the rows after
//! the first are laid out again, narrower, and shifted right.
use egui::text::{LayoutJob, LayoutSection};
use egui::{Galley, Rect, Ui};
use egui::epaint::text::ByteIndex;
use std::sync::Arc;

/// The part of `job` between byte `start` and `end` (no newline), as its own job.
fn sub_job(job: &LayoutJob, start: usize, end: usize, wrap: f32) -> LayoutJob {
    let mut out = LayoutJob { text: job.text[start..end].to_owned(), ..Default::default() };
    out.wrap = job.wrap.clone();
    out.wrap.max_width = wrap;
    out.halign = job.halign;
    out.break_on_newline = job.break_on_newline;
    out.round_output_to_gui = job.round_output_to_gui;
    out.keep_trailing_whitespace = job.keep_trailing_whitespace;
    for s in &job.sections {
        let (a, b) = (s.byte_range.start.0, s.byte_range.end.0);
        // Keep sections that overlap, plus an empty-range one sitting at `end` (the newline's
        // format) so an empty line still gets its height.
        if b <= start && !(a == start && b == start) {
            continue;
        }
        if a > end {
            break;
        }
        out.sections.push(LayoutSection {
            leading_space: if a >= start { s.leading_space } else { 0.0 },
            byte_range: ByteIndex(a.max(start) - start)..ByteIndex(b.min(end).max(a.max(start)) - start),
            format: s.format.clone(),
        });
    }
    out
}

pub fn recount(g: &mut Galley) {
    let mut rect = Rect::NOTHING;
    let mut mesh = Rect::NOTHING;
    let (mut v, mut i) = (0, 0);
    for r in &g.rows {
        rect |= r.rect();
        mesh |= r.visuals.mesh_bounds.translate(r.pos.to_vec2());
        v += r.visuals.mesh.vertices.len();
        i += r.visuals.mesh.indices.len();
    }
    g.rect = rect;
    g.mesh_bounds = mesh;
    g.num_vertices = v;
    g.num_indices = i;
}

/// Lay out `job` line by line. `hang[i]` is the byte offset in line `i` where its text starts
/// (after the indent and marker); its wrapped rows start at that x.
pub fn layout(ui: &Ui, job: LayoutJob, hang: &[Option<usize>]) -> Arc<Galley> {
    let w = job.wrap.max_width;
    if !w.is_finite() || !hang.iter().any(|h| h.is_some()) {
        return ui.fonts_mut(|f| f.layout_job(job));
    }
    let text = job.text.clone();
    let mut parts: Vec<Arc<Galley>> = Vec::new();
    let mut start = 0;
    for (li, line) in text.split('\n').enumerate() {
        let end = start + line.len();
        let g = ui.fonts_mut(|f| f.layout_job(sub_job(&job, start, end, w)));
        let mut part = g.clone();
        if let (Some(cb), true) = (hang.get(li).copied().flatten(), g.rows.len() > 1) {
            let ci = line[..cb.min(line.len())].chars().count();
            let r0 = &g.rows[0];
            let p = r0.glyphs.get(ci).map(|gl| gl.pos.x).unwrap_or(0.0);
            let k = r0.char_count_excluding_newline().0;
            if p > 0.5 && p < w * 0.5 && ci < k {
                let kb = line.char_indices().nth(k).map(|(b, _)| b).unwrap_or(line.len());
                let rest = ui.fonts_mut(|f| f.layout_job(sub_job(&job, start + kb, end, (w - p).max(40.0))));
                let mut m = (*g).clone();
                let y0 = g.rows[1].pos.y;
                m.rows.truncate(1);
                if let Some(r) = m.rows.first_mut() {
                    r.ends_with_newline = false;
                }
                for r in &rest.rows {
                    let mut r = r.clone();
                    r.pos.x += p;
                    r.pos.y += y0;
                    m.rows.push(r);
                }
                recount(&mut m);
                part = Arc::new(m);
            }
        }
        parts.push(part);
        start = end + 1;
    }
    let ppp = ui.ctx().pixels_per_point();
    Arc::new(Galley::concat(Arc::new(job), &parts, ppp))
}

/// Just rows `rows` of `g`, at their places in it, to paint a piece of the text on its own.
pub fn rows_galley(g: &Galley, rows: std::ops::Range<usize>) -> Arc<Galley> {
    let mut out = g.clone();
    out.rows = g.rows[rows].to_vec();
    recount(&mut out);
    Arc::new(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Color32, FontId, TextFormat};

    #[test]
    fn sub_job_keeps_empty_line_format() {
        let mut job = LayoutJob::default();
        let f = TextFormat::simple(FontId::proportional(16.0), Color32::WHITE);
        job.append("ab", 0.0, f.clone());
        job.append("\n", 0.0, f.clone());
        job.append("", 0.0, f.clone());
        job.append("\n", 0.0, f.clone());
        job.append("cd", 0.0, f);
        let s = sub_job(&job, 3, 3, 100.0);
        assert_eq!(s.text, "");
        assert!(!s.sections.is_empty());
        let s = sub_job(&job, 4, 6, 100.0);
        assert_eq!(s.text, "cd");
        assert_eq!(s.sections.last().unwrap().byte_range, ByteIndex(0)..ByteIndex(2));
    }
}
