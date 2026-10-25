use std::ffi::OsString;
use std::path::{Path, PathBuf};

use vellum_grid_core::{Sheet, Workbook};

use crate::{files, text};

pub enum Launch {
    Open(Option<PathBuf>),
    Exit,
}

pub fn launch(args: Vec<OsString>) -> Launch {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Launch::Open(None);
    };
    match first.to_str() {
        Some("--blank") => {
            if let Some(path) = args.next() {
                let _ = write_blank(Path::new(&path));
            }
            Launch::Exit
        }
        Some("--new") => {
            let dir = args
                .next()
                .map_or_else(|| PathBuf::from("."), PathBuf::from);
            create(&unused_path(&dir, text::new_file_name()))
        }
        Some("--new-file") => {
            let Some(requested) = args.next().map(PathBuf::from) else {
                return Launch::Open(None);
            };
            let placeholder = std::fs::metadata(&requested).is_ok_and(|m| m.len() == 0);
            if placeholder && requested.extension().is_some_and(|e| e != "xlsx") {
                let _ = std::fs::remove_file(&requested);
            }
            let dir = requested
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
            let stem = requested.file_stem().map_or_else(
                || text::new_file_name().to_owned(),
                |s| s.to_string_lossy().into_owned(),
            );
            create(&unused_path(&dir, &stem))
        }
        _ => Launch::Open(Some(PathBuf::from(first))),
    }
}

fn write_blank(path: &Path) -> Result<(), String> {
    let book =
        Workbook::from_sheets(vec![Sheet::new(text::new_sheet())]).map_err(|e| e.to_string())?;
    let bytes = files::write(&book, 0, path)?;
    files::store(path, &bytes)
}

fn create(path: &Path) -> Launch {
    match write_blank(path) {
        Ok(()) => Launch::Open(Some(path.to_path_buf())),
        Err(_) => Launch::Open(None),
    }
}

fn unused_path(dir: &Path, stem: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.xlsx"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}).xlsx")))
        .find(|path| !path.exists())
        .expect("an unused name exists")
}

#[cfg(target_os = "linux")]
pub fn ensure_template() {
    let Some(marker) = crate::settings::app_dir().map(|dir| dir.join("template-installed")) else {
        return;
    };
    if marker.exists() {
        return;
    }
    let output = std::process::Command::new("xdg-user-dir")
        .arg("TEMPLATES")
        .output();
    let Ok(output) = output else {
        return;
    };
    let dir = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if !dir.is_dir() || Some(&dir) == home.as_ref() {
        return;
    }
    let template = dir.join(format!("{}.xlsx", text::template_name()));
    if template.exists() || write_blank(&template).is_ok() {
        if let Some(parent) = marker.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&marker, "");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_file_replaces_the_placeholder_with_a_workbook() {
        let dir = std::env::temp_dir().join("vellum-grid-shell-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let requested = dir.join("Yeni Vellum Grid Çalışma Sayfası.vellumgrid");
        std::fs::write(&requested, "").unwrap();
        let args = vec!["--new-file".into(), requested.clone().into_os_string()];
        let Launch::Open(Some(first)) = launch(args.clone()) else {
            panic!("a workbook is opened");
        };
        assert_eq!(first, dir.join("Yeni Vellum Grid Çalışma Sayfası.xlsx"));
        assert!(!requested.exists());
        assert!(std::fs::metadata(&first).unwrap().len() > 0);
        let Launch::Open(Some(second)) = launch(args) else {
            panic!("a workbook is opened");
        };
        assert_eq!(
            second,
            dir.join("Yeni Vellum Grid Çalışma Sayfası (2).xlsx")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
