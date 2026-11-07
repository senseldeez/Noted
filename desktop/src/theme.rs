//! Colours and fonts: the same OLED palette as the Android app.
use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId};
use std::sync::Arc;

pub const BLACK: Color32 = Color32::BLACK;
pub const WHITE: Color32 = Color32::WHITE;
pub const BORDER: Color32 = Color32::from_gray(0x33);
pub const BORDER_HOVER: Color32 = Color32::from_gray(0x4a);
pub const DIVIDER: Color32 = Color32::from_gray(0x22);
pub const PILL: Color32 = Color32::from_gray(0x16);
pub const TOOLBAR: Color32 = Color32::from_gray(0x1c);
pub const TOOLBAR_HOVER: Color32 = Color32::from_gray(0x2a);
pub const TEXT2: Color32 = Color32::from_gray(0xd6);
pub const TEXT3: Color32 = Color32::from_gray(0xb5);
pub const MUTED: Color32 = Color32::from_gray(0x8f);
pub const SYNTAX: Color32 = Color32::from_gray(0x7a);
pub const GUIDE: Color32 = Color32::from_gray(0x2e);
pub const CODE: Color32 = Color32::from_gray(0x0e);
pub const SURFACE: Color32 = Color32::from_gray(0x11);
pub const MENU: Color32 = Color32::from_gray(0x14);
pub const MENU_HOVER: Color32 = Color32::from_gray(0x22);
pub const LINE: f32 = 2.0;

pub fn sans(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}
pub fn xbold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("xbold".into()))
}
pub fn italic(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("italic".into()))
}
pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

pub fn fonts() -> FontDefinitions {
    let mut f = FontDefinitions::default();
    let add = |f: &mut FontDefinitions, name: &str, bytes: &'static [u8]| {
        f.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    };
    add(&mut f, "inter", include_bytes!("../assets/fonts/Inter-Regular.ttf"));
    add(&mut f, "inter-semibold", include_bytes!("../assets/fonts/Inter-SemiBold.ttf"));
    add(&mut f, "inter-bold", include_bytes!("../assets/fonts/Inter-Bold.ttf"));
    add(&mut f, "inter-xbold", include_bytes!("../assets/fonts/Inter-ExtraBold.ttf"));
    add(&mut f, "inter-italic", include_bytes!("../assets/fonts/Inter-Italic.ttf"));
    add(&mut f, "jbm", include_bytes!("../assets/fonts/jetbrains_mono_regular.ttf"));
    // Symbols the text fonts lack fall back to egui's bundled fonts.
    let fallback: Vec<String> = f.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let fam = |first: &str| {
        let mut v = vec![first.to_string()];
        v.extend(fallback.iter().cloned());
        v
    };
    f.families.insert(FontFamily::Proportional, fam("inter"));
    f.families.insert(FontFamily::Monospace, fam("jbm"));
    f.families.insert(FontFamily::Name("semibold".into()), fam("inter-semibold"));
    f.families.insert(FontFamily::Name("bold".into()), fam("inter-bold"));
    f.families.insert(FontFamily::Name("xbold".into()), fam("inter-xbold"));
    f.families.insert(FontFamily::Name("italic".into()), fam("inter-italic"));
    f
}

pub fn style(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    let mut v = egui::Visuals::dark();
    v.panel_fill = BLACK;
    v.window_fill = SURFACE;
    v.extreme_bg_color = BLACK;
    v.faint_bg_color = PILL;
    v.override_text_color = Some(TEXT2);
    v.selection.bg_fill = Color32::from_gray(0x44);
    v.selection.stroke = egui::Stroke::new(1.0, WHITE);
    v.text_cursor.stroke = egui::Stroke::new(2.0, WHITE);
    v.hyperlink_color = WHITE;
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(LINE, BORDER);
    v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.hovered.weak_bg_fill = PILL;
    v.popup_shadow = egui::epaint::Shadow::NONE;
    v.window_shadow = egui::epaint::Shadow::NONE;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(0.0, 0.0);
        s.spacing.scroll.bar_width = 6.0;
        s.spacing.scroll.floating = true;
        s.interaction.tooltip_delay = 0.38;
        s.text_styles.insert(egui::TextStyle::Body, sans(14.0));
        s.text_styles.insert(egui::TextStyle::Button, sans(14.0));
    });
}
