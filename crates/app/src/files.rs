use std::path::{Path, PathBuf};

use vellum_grid_core::{Workbook, sheet_name_from};
use vellum_grid_io::{book, csv, html, ods, tsv};

use crate::text;

const READABLE: [&str; 7] = ["xlsx", "xlsm", "xls", "xlsb", "ods", "csv", "txt"];
const WRITABLE: [&str; 5] = ["xlsx", "ods", "csv", "txt", "html"];
const BYTE_ORDER_MARK: &str = "\u{FEFF}";
const DEFAULT_EXTENSION: &str = "xlsx";

pub type Opened = (PathBuf, Vec<u8>);

pub fn book_name(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| text::new_book().to_owned())
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn can_save(path: &Path) -> bool {
    WRITABLE.contains(&extension(path).as_str())
}

pub fn is_csv(path: &Path) -> bool {
    extension(path) == "csv"
}

pub fn is_pdf(path: &Path) -> bool {
    extension(path) == "pdf"
}

pub fn single_sheet(path: &Path) -> bool {
    matches!(extension(path).as_str(), "csv" | "txt" | "html")
}

pub fn read(path: &Path, bytes: Vec<u8>) -> Result<Workbook, String> {
    if is_csv(path) || extension(path) == "txt" {
        let name =
            sheet_name_from(&book_name(path)).unwrap_or_else(|| text::new_sheet().to_owned());
        let sheet = csv::read_bytes(&name, &bytes).map_err(|e| e.to_string())?;
        Workbook::from_sheets(vec![sheet]).map_err(|e| e.to_string())
    } else {
        book::read_workbook(bytes).map_err(|e| e.to_string())
    }
}

pub fn write(book: &Workbook, active: usize, path: &Path) -> Result<Vec<u8>, String> {
    if is_csv(path) {
        let mut out = Vec::new();
        csv::write_sheet(&book.sheets()[active], &mut out).map_err(|e| e.to_string())?;
        Ok(out)
    } else if extension(path) == "ods" {
        ods::write_ods(book).map_err(|e| e.to_string())
    } else if extension(path) == "txt" {
        let sheet = &book.sheets()[active];
        let (rows, cols) = sheet.extent();
        let text = match (rows, cols) {
            (0, _) | (_, 0) => String::new(),
            _ => {
                let end = vellum_grid_core::CellAddr::new(rows - 1, cols - 1)
                    .expect("extent is inside the sheet");
                let range = vellum_grid_core::CellRange::new(crate::app::origin(), end);
                tsv::to_tsv(sheet, range, |cell| {
                    crate::app::locale().format_value(&cell.value, cell.number_format())
                })
            }
        };
        Ok(format!("{BYTE_ORDER_MARK}{text}").into_bytes())
    } else if extension(path) == "html" {
        let sheet = &book.sheets()[active];
        Ok(html::write_html(sheet, sheet.name(), crate::app::locale()).into_bytes())
    } else {
        book::write_xlsx(book).map_err(|e| e.to_string())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn with_extension(path: PathBuf) -> PathBuf {
    if path.extension().is_some() {
        path
    } else {
        path.with_extension(DEFAULT_EXTENSION)
    }
}

const IMAGES: [&str; 6] = ["png", "jpg", "jpeg", "gif", "bmp", "dib"];

#[derive(Default)]
pub struct Inbox {
    #[cfg(target_arch = "wasm32")]
    slot: std::rc::Rc<std::cell::RefCell<Option<Opened>>>,
    #[cfg(target_arch = "wasm32")]
    image: std::rc::Rc<std::cell::RefCell<Option<Vec<u8>>>>,
}

impl Inbox {
    pub fn poll_image(&self) -> Option<Vec<u8>> {
        #[cfg(target_arch = "wasm32")]
        {
            self.image.borrow_mut().take()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            None
        }
    }

    pub fn poll(&self) -> Option<Opened> {
        #[cfg(target_arch = "wasm32")]
        {
            self.slot.borrow_mut().take()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            None
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn pick_open(_inbox: &Inbox, _ctx: &eframe::egui::Context) -> Option<Opened> {
    let path = rfd::FileDialog::new()
        .add_filter(text::spreadsheet_filter(), &READABLE)
        .add_filter(text::xlsx_filter(), &["xlsx", "xlsm"])
        .add_filter(text::ods_filter(), &["ods"])
        .add_filter(text::csv_filter(), &["csv"])
        .add_filter(text::txt_filter(), &["txt"])
        .pick_file()?;
    let bytes = std::fs::read(&path).ok()?;
    Some((path, bytes))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn pick_image(_inbox: &Inbox, _ctx: &eframe::egui::Context) -> Option<Vec<u8>> {
    let path = rfd::FileDialog::new()
        .add_filter(text::image_filter(), &IMAGES)
        .pick_file()?;
    std::fs::read(path).ok()
}

#[cfg(target_arch = "wasm32")]
pub fn pick_image(inbox: &Inbox, ctx: &eframe::egui::Context) -> Option<Vec<u8>> {
    let slot = inbox.image.clone();
    let ctx = ctx.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let dialog = rfd::AsyncFileDialog::new().add_filter(text::image_filter(), &IMAGES);
        if let Some(file) = dialog.pick_file().await {
            *slot.borrow_mut() = Some(file.read().await);
            ctx.request_repaint();
        }
    });
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub fn pick_text() -> Option<Opened> {
    let path = rfd::FileDialog::new()
        .add_filter(text::text_files_filter(), &["csv", "txt", "tsv", "prn"])
        .pick_file()?;
    let bytes = std::fs::read(&path).ok()?;
    Some((path, bytes))
}

#[cfg(target_arch = "wasm32")]
pub fn pick_text() -> Option<Opened> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub fn pick_save(book: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter(text::xlsx_filter(), &["xlsx"])
        .add_filter(text::ods_filter(), &["ods"])
        .add_filter(text::csv_filter(), &["csv"])
        .add_filter(text::txt_filter(), &["txt"])
        .add_filter(text::html_filter(), &["html"])
        .add_filter(text::pdf_filter(), &["pdf"])
        .set_file_name(format!("{book}.{DEFAULT_EXTENSION}"))
        .save_file()
        .map(with_extension)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn pick_export(book: &str, extension: &str) -> Option<PathBuf> {
    let filter = match extension {
        "pdf" => text::pdf_filter(),
        "ods" => text::ods_filter(),
        "csv" => text::csv_filter(),
        "txt" => text::txt_filter(),
        "html" => text::html_filter(),
        _ => text::xlsx_filter(),
    };
    rfd::FileDialog::new()
        .add_filter(filter, &[extension])
        .set_file_name(format!("{book}.{extension}"))
        .save_file()
        .map(|path| {
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case(extension))
            {
                path
            } else {
                path.with_extension(extension)
            }
        })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load(path: &Path) -> Result<Workbook, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    read(path, bytes)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn store(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

#[cfg(target_arch = "wasm32")]
pub fn pick_open(inbox: &Inbox, ctx: &eframe::egui::Context) -> Option<Opened> {
    let slot = inbox.slot.clone();
    let ctx = ctx.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let dialog = rfd::AsyncFileDialog::new().add_filter(text::spreadsheet_filter(), &READABLE);
        if let Some(file) = dialog.pick_file().await {
            let bytes = file.read().await;
            *slot.borrow_mut() = Some((PathBuf::from(file.file_name()), bytes));
            ctx.request_repaint();
        }
    });
    None
}

#[cfg(target_arch = "wasm32")]
pub fn pick_save(book: &str) -> Option<PathBuf> {
    Some(PathBuf::from(format!("{book}.{DEFAULT_EXTENSION}")))
}

#[cfg(target_arch = "wasm32")]
pub fn pick_export(book: &str, extension: &str) -> Option<PathBuf> {
    Some(PathBuf::from(format!("{book}.{extension}")))
}

#[cfg(target_arch = "wasm32")]
pub fn load(_path: &Path) -> Result<Workbook, String> {
    Err(text::open_failed().to_owned())
}

#[cfg(target_arch = "wasm32")]
pub fn store(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use eframe::wasm_bindgen::JsCast;
    use eframe::web_sys::{Blob, HtmlAnchorElement, Url};

    let fail = |_| text::save_failed().to_owned();
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let blob = Blob::new_with_u8_array_sequence(&parts).map_err(fail)?;
    let url = Url::create_object_url_with_blob(&blob).map_err(fail)?;
    let document = eframe::web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| text::save_failed().to_owned())?;
    let anchor: HtmlAnchorElement = document
        .create_element("a")
        .map_err(fail)?
        .dyn_into()
        .map_err(|_| text::save_failed().to_owned())?;
    anchor.set_href(&url);
    anchor.set_download(&path.to_string_lossy());
    anchor.click();
    Url::revoke_object_url(&url).map_err(fail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vellum_grid_core::Sheet;

    #[test]
    fn writes_text_and_web_page_from_the_active_sheet() {
        let mut sheet = Sheet::new("Veri");
        sheet.set_input("A1".parse().unwrap(), "Ad");
        sheet.set_input("B1".parse().unwrap(), "1234.5");
        let book = Workbook::from_sheets(vec![sheet]).unwrap();
        let txt = write(&book, 0, Path::new("a.txt")).unwrap();
        assert_eq!(String::from_utf8(txt).unwrap(), "\u{FEFF}Ad\t1234,5\r\n");
        let html = String::from_utf8(write(&book, 0, Path::new("a.html")).unwrap()).unwrap();
        assert!(html.contains("<title>Veri</title>"));
        assert!(single_sheet(Path::new("a.HTML")));
        assert!(is_pdf(Path::new("rapor.pdf")));
        assert!(can_save(Path::new("a.txt")));
    }
}
