use std::path::{Path, PathBuf};

use vellum_grid_core::Workbook;

#[derive(Debug, Clone)]
pub struct Recovered {
    pub path: PathBuf,
    pub name: String,
    pub saved: String,
}

fn dir() -> Option<PathBuf> {
    Some(crate::settings::app_dir()?.join("Recovery"))
}

pub fn file_for(book: &str) -> Option<PathBuf> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    let safe: String = book
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    Some(dir()?.join(format!("{safe}~{stamp}.xlsx")))
}

pub fn list() -> Vec<Recovered> {
    let Some(entries) = dir().and_then(|dir| std::fs::read_dir(dir).ok()) else {
        return Vec::new();
    };
    let mut found: Vec<(std::time::SystemTime, Recovered)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "xlsx"))
        .filter_map(|path| {
            let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok()?;
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            let name = stem
                .rsplit_once('~')
                .map_or(stem.clone(), |(name, _)| name.to_owned());
            let saved = crate::settings::ago(modified);
            Some((modified, Recovered { path, name, saved }))
        })
        .collect();
    found.sort_by_key(|(time, _)| std::cmp::Reverse(*time));
    found.into_iter().map(|(_, recovered)| recovered).collect()
}

pub fn write(book: Workbook, active: usize, path: PathBuf) {
    let job = move || {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(bytes) = crate::files::write(&book, active, &path) {
            let _ = crate::files::store(&path, &bytes);
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(job);
    #[cfg(target_arch = "wasm32")]
    drop(job);
}

pub fn remove(path: &Path) {
    let _ = std::fs::remove_file(path);
}
