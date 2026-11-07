#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assist;
mod backstage;
mod cells;
mod chart_ui;
mod charts;
mod clock;
mod conditional_ui;
mod context;
mod data;
mod data_tools;
mod dialogs;
mod error_check;
mod files;
mod filter_ui;
mod fluent;
mod fonts;
mod format;
mod format_cells;
mod function_help;
mod geometry;
mod goto;
mod grid;
mod icons;
mod insert_function;
mod keys;
mod links_ui;
mod logo;
mod menu;
mod names;
mod objects;
mod objects_ui;
mod page_setup;
mod palette;
mod paste;
use vellum_ui::printers;
mod printing;
mod protection_ui;
mod recovery;
mod references;
mod render;
mod review_ui;
mod ribbon;
mod search;
mod settings;
mod shell;
mod sparklines_ui;
mod spelling;
mod status;
mod tables_ui;
mod tabs;
mod text;
mod theme;
mod titlebar;
mod trace;
mod validation_ui;

fn main() -> eframe::Result {
    #[cfg(target_os = "linux")]
    shell::ensure_template();
    let path = match shell::launch(std::env::args_os().skip(1).collect()) {
        shell::Launch::Open(path) => path,
        shell::Launch::Exit => return Ok(()),
    };
    fonts::prefetch();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(text::app_name())
            .with_inner_size([theme::WINDOW_WIDTH, theme::WINDOW_HEIGHT])
            .with_min_inner_size([theme::WINDOW_MIN_WIDTH, theme::WINDOW_MIN_HEIGHT])
            .with_decorations(false)
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!(
                    "../../../assets/vellum-grid-256.png"
                ))
                .expect("bundled icon is a valid png"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        text::app_name(),
        options,
        Box::new(|cc| Ok(Box::new(app::Spreadsheet::new(cc, path)))),
    )
}
