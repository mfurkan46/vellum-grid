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
mod printers;
mod printing;
mod protection_ui;
mod recovery;
mod references;
mod render;
mod review_ui;
mod ribbon;
mod search;
mod settings;
#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    #[cfg(target_os = "linux")]
    shell::ensure_template();
    let path = match shell::launch(std::env::args_os().skip(1).collect()) {
        shell::Launch::Open(path) => path,
        shell::Launch::Exit => return Ok(()),
    };
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

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast;
    use eframe::web_sys::HtmlCanvasElement;

    wasm_bindgen_futures::spawn_local(async {
        let canvas = eframe::web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id("vellum-grid"))
            .and_then(|element| element.dyn_into::<HtmlCanvasElement>().ok())
            .expect("page has a canvas with id 'vellum-grid'");
        eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(app::Spreadsheet::new(cc, None)))),
            )
            .await
            .expect("eframe starts");
    });
}
