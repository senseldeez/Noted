#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]
mod app;
mod editor;
mod grid;
mod icons;
mod md;
mod reader;
mod store;
mod theme;
mod widgets;

fn icon() -> egui::IconData {
    let img = image::load_from_memory(include_bytes!("../assets/icon.png")).expect("icon").into_rgba8();
    let img = image::imageops::resize(&img, 256, 256, image::imageops::FilterType::Lanczos3);
    let (w, h) = img.dimensions();
    egui::IconData { rgba: img.into_raw(), width: w, height: h }
}

fn main() -> eframe::Result {
    let mut vp = egui::ViewportBuilder::default()
        .with_title("Noted")
        .with_app_id("noted")
        .with_inner_size([1240.0, 840.0])
        .with_min_inner_size([560.0, 480.0])
        .with_icon(icon());
    if cfg!(target_os = "macos") {
        vp = vp.with_fullsize_content_view(true).with_title_shown(false).with_titlebar_shown(false);
    } else {
        // The app draws its own title bar and window buttons, like the Electron version did.
        vp = vp.with_decorations(false);
    }
    let opts = eframe::NativeOptions { viewport: vp, persist_window: true, ..Default::default() };
    eframe::run_native("Noted", opts, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
