//! Line icons drawn with the painter, matching the 2.3px rounded strokes of the Android app.
use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Back,
    Plus,
    Search,
    More,
    Folder,
    FolderPlus,
    Pin,
    Edit,
    Read,
    Trash,
    Move,
    Rename,
    Close,
    Chevron,
    Bullets,
    Numbers,
    Checkbox,
    Quote,
    Code,
    Link,
    Rule,
    Refresh,
    Keys,
    Reveal,
    Indent,
    Outdent,
    Text,
    Import,
    Undo,
    Redo,
    PinFill,
    FolderFill,
    Note,
    Check,
    Minimize,
    Maximize,
    Restore,
}

/// Draw `icon` centred in `rect`, designed on a 24-unit grid.
pub fn paint(p: &Painter, rect: Rect, icon: Icon, color: Color32) {
    paint_rot(p, rect, icon, color, 0.0)
}

pub fn paint_rot(p: &Painter, rect: Rect, icon: Icon, color: Color32, angle: f32) {
    let s = rect.width().min(rect.height()) / 24.0;
    let c = rect.center();
    let (sin, cos) = angle.sin_cos();
    let at = |x: f32, y: f32| -> Pos2 {
        let (dx, dy) = ((x - 12.0) * s, (y - 12.0) * s);
        pos2(c.x + dx * cos - dy * sin, c.y + dx * sin + dy * cos)
    };
    let w = (2.2 * s).max(1.4);
    let st = Stroke::new(w, color);
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|&(x, y)| at(x, y)).collect(), st));
        for &(x, y) in [pts[0], pts[pts.len() - 1]].iter() {
            p.circle_filled(at(x, y), w / 2.0, color);
        }
    };
    let circle = |x: f32, y: f32, r: f32| {
        p.circle_stroke(at(x, y), r * s, st);
    };
    let dot = |x: f32, y: f32, r: f32| {
        p.circle_filled(at(x, y), r * s, color);
    };
    let rrect = |x0: f32, y0: f32, x1: f32, y1: f32, r: f32| {
        let rc = Rect::from_two_pos(at(x0, y0), at(x1, y1));
        p.rect_stroke(rc, r * s, st, egui::StrokeKind::Middle);
    };
    let folder = || {
        line(&[(3.0, 7.0), (3.0, 18.0), (4.0, 19.0), (20.0, 19.0), (21.0, 18.0), (21.0, 9.0), (20.0, 8.0), (12.0, 8.0), (10.0, 5.0), (4.0, 5.0), (3.0, 6.0), (3.0, 7.0)]);
    };
    match icon {
        Icon::Back => line(&[(15.0, 5.0), (8.0, 12.0), (15.0, 19.0)]),
        Icon::Chevron => line(&[(9.0, 5.0), (16.0, 12.0), (9.0, 19.0)]),
        Icon::Plus => {
            line(&[(12.0, 5.0), (12.0, 19.0)]);
            line(&[(5.0, 12.0), (19.0, 12.0)]);
        }
        Icon::Close => {
            line(&[(6.0, 6.0), (18.0, 18.0)]);
            line(&[(18.0, 6.0), (6.0, 18.0)]);
        }
        Icon::Search => {
            circle(10.5, 10.5, 6.0);
            line(&[(15.0, 15.0), (20.0, 20.0)]);
        }
        Icon::More => {
            dot(5.5, 12.0, 1.9);
            dot(12.0, 12.0, 1.9);
            dot(18.5, 12.0, 1.9);
        }
        Icon::Folder => folder(),
        Icon::FolderFill => {
            let fill = |x0: f32, y0: f32, x1: f32, y1: f32, r: f32| {
                p.rect_filled(Rect::from_two_pos(at(x0, y0), at(x1, y1)), r * s, color);
            };
            fill(3.0, 5.0, 11.0, 10.0, 2.0);
            fill(3.0, 7.5, 21.0, 19.0, 2.0);
            folder();
        }
        Icon::FolderPlus => {
            folder();
            line(&[(12.0, 11.0), (12.0, 16.0)]);
            line(&[(9.5, 13.5), (14.5, 13.5)]);
        }
        Icon::Pin | Icon::PinFill => {
            if icon == Icon::PinFill {
                let poly = |pts: &[(f32, f32)]| {
                    p.add(Shape::convex_polygon(pts.iter().map(|&(x, y)| at(x, y)).collect(), color, Stroke::NONE));
                };
                poly(&[(9.0, 3.0), (15.0, 3.0), (14.0, 10.0), (10.0, 10.0)]);
                poly(&[(10.0, 10.0), (14.0, 10.0), (18.0, 14.0), (6.0, 14.0)]);
            }
            line(&[(9.0, 3.0), (15.0, 3.0), (14.0, 10.0), (18.0, 14.0), (6.0, 14.0), (10.0, 10.0), (9.0, 3.0)]);
            line(&[(12.0, 14.0), (12.0, 21.0)]);
        }
        Icon::Edit => {
            line(&[(4.0, 20.0), (4.0, 16.0), (15.0, 5.0), (19.0, 9.0), (8.0, 20.0), (4.0, 20.0)]);
            line(&[(13.0, 7.0), (17.0, 11.0)]);
        }
        Icon::Read => {
            // Eye.
            let mut pts = Vec::new();
            for i in 0..=24 {
                let t = i as f32 / 24.0;
                pts.push((2.0 + 20.0 * t, 12.0 - 7.0 * (std::f32::consts::PI * t).sin()));
            }
            for i in 1..=24 {
                let t = 1.0 - i as f32 / 24.0;
                pts.push((2.0 + 20.0 * t, 12.0 + 7.0 * (std::f32::consts::PI * t).sin()));
            }
            line(&pts);
            circle(12.0, 12.0, 3.0);
        }
        Icon::Trash => {
            line(&[(4.0, 7.0), (20.0, 7.0)]);
            line(&[(9.0, 7.0), (9.0, 4.0), (15.0, 4.0), (15.0, 7.0)]);
            line(&[(6.0, 7.0), (7.0, 20.0), (17.0, 20.0), (18.0, 7.0)]);
        }
        Icon::Move => {
            folder();
            line(&[(9.0, 13.5), (15.0, 13.5)]);
            line(&[(12.5, 11.0), (15.0, 13.5), (12.5, 16.0)]);
        }
        Icon::Rename => {
            line(&[(4.0, 20.0), (20.0, 20.0)]);
            line(&[(6.0, 16.0), (6.0, 13.0), (15.0, 4.0), (18.0, 7.0), (9.0, 16.0), (6.0, 16.0)]);
        }
        Icon::Bullets => {
            for y in [6.0, 12.0, 18.0] {
                dot(4.5, y, 1.6);
                line(&[(9.0, y), (20.0, y)]);
            }
        }
        Icon::Numbers => {
            for y in [6.0, 12.0, 18.0] {
                line(&[(10.0, y), (20.0, y)]);
            }
            line(&[(4.0, 5.0), (5.5, 4.0), (5.5, 8.5)]);
            line(&[(3.5, 15.5), (5.0, 14.5), (6.5, 15.5), (3.5, 19.5), (7.0, 19.5)]);
        }
        Icon::Checkbox => {
            rrect(4.0, 4.0, 20.0, 20.0, 4.0);
            line(&[(8.0, 12.0), (11.0, 15.0), (16.5, 9.0)]);
        }
        Icon::Quote => {
            line(&[(5.0, 5.0), (5.0, 19.0)]);
            line(&[(10.0, 8.0), (20.0, 8.0)]);
            line(&[(10.0, 12.0), (20.0, 12.0)]);
            line(&[(10.0, 16.0), (16.0, 16.0)]);
        }
        Icon::Code => {
            line(&[(8.0, 7.0), (3.0, 12.0), (8.0, 17.0)]);
            line(&[(16.0, 7.0), (21.0, 12.0), (16.0, 17.0)]);
            line(&[(13.5, 5.0), (10.5, 19.0)]);
        }
        Icon::Link => {
            line(&[(10.0, 14.0), (14.0, 10.0)]);
            line(&[(11.0, 7.0), (13.5, 4.5), (16.0, 4.0), (19.5, 7.5), (19.5, 9.0), (17.0, 13.0)]);
            line(&[(13.0, 17.0), (10.5, 19.5), (8.0, 20.0), (4.5, 16.5), (4.5, 15.0), (7.0, 11.0)]);
        }
        Icon::Rule => {
            line(&[(3.0, 12.0), (21.0, 12.0)]);
        }
        Icon::Refresh => {
            let mut pts = Vec::new();
            for i in 0..=20 {
                let a = std::f32::consts::PI * (0.15 + 1.6 * i as f32 / 20.0);
                pts.push((12.0 + 7.0 * a.cos(), 12.0 + 7.0 * a.sin()));
            }
            line(&pts);
            line(&[(16.0, 4.0), (18.5, 6.0), (16.5, 8.5)]);
        }
        Icon::Keys => {
            rrect(2.5, 6.0, 21.5, 18.0, 3.0);
            for x in [6.5, 10.0, 13.5, 17.0] {
                dot(x, 10.0, 1.1);
            }
            line(&[(8.0, 14.0), (16.0, 14.0)]);
        }
        Icon::Reveal => {
            rrect(3.0, 5.0, 21.0, 19.0, 3.0);
            line(&[(3.0, 9.0), (21.0, 9.0)]);
            dot(6.0, 7.0, 0.9);
            dot(9.0, 7.0, 0.9);
        }
        Icon::Indent | Icon::Outdent => {
            line(&[(11.0, 6.0), (20.0, 6.0)]);
            line(&[(11.0, 12.0), (20.0, 12.0)]);
            line(&[(11.0, 18.0), (20.0, 18.0)]);
            if icon == Icon::Indent {
                line(&[(4.0, 9.0), (7.0, 12.0), (4.0, 15.0)]);
            } else {
                line(&[(7.0, 9.0), (4.0, 12.0), (7.0, 15.0)]);
            }
        }
        Icon::Text => {
            line(&[(5.0, 6.0), (19.0, 6.0)]);
            line(&[(12.0, 6.0), (12.0, 19.0)]);
        }
        Icon::Undo | Icon::Redo => {
            let m = |x: f32| if icon == Icon::Undo { x } else { 24.0 - x };
            line(&[(m(9.0), 14.0), (m(4.0), 9.0), (m(9.0), 4.0)]);
            let mut pts = vec![(m(4.0), 9.0), (m(14.0), 9.0)];
            for i in 1..=16 {
                let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 16.0;
                pts.push((m(14.0 + 6.0 * a.cos()), 15.0 + 6.0 * a.sin()));
            }
            pts.push((m(11.0), 21.0));
            line(&pts);
        }
        Icon::Note => {
            line(&[(7.0, 3.0), (14.0, 3.0), (19.0, 8.0), (19.0, 21.0), (7.0, 21.0), (5.0, 19.0), (5.0, 5.0), (7.0, 3.0)]);
            line(&[(14.0, 3.0), (14.0, 8.0), (19.0, 8.0)]);
        }
        Icon::Check => line(&[(6.0, 12.5), (10.0, 16.5), (18.0, 7.5)]),
        Icon::Minimize => line(&[(5.0, 12.0), (19.0, 12.0)]),
        Icon::Maximize => rrect(5.0, 5.0, 19.0, 19.0, 2.0),
        Icon::Restore => {
            rrect(5.0, 8.0, 16.0, 19.0, 2.0);
            line(&[(8.0, 5.0), (19.0, 5.0), (19.0, 16.0)]);
        }
        Icon::Import => {
            line(&[(12.0, 4.0), (12.0, 15.0)]);
            line(&[(7.5, 10.5), (12.0, 15.0), (16.5, 10.5)]);
            line(&[(4.0, 16.0), (4.0, 20.0), (20.0, 20.0), (20.0, 16.0)]);
        }
    }
    let _ = vec2;
}
