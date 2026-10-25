use std::path::{Path, PathBuf};

pub use vellum_ui::settings::ago;

const DICTIONARY_FILE: &str = "dictionary.txt";
const APP: &str = "Vellum Grid";

pub fn app_dir() -> Option<PathBuf> {
    vellum_ui::settings::app_dir(APP)
}

pub fn recent() -> Vec<PathBuf> {
    vellum_ui::settings::recent(APP)
}

pub fn remember(path: &Path) {
    vellum_ui::settings::remember(APP, path);
}

pub fn forget(path: &Path) {
    vellum_ui::settings::forget(APP, path);
}

pub fn user_words() -> Vec<String> {
    app_dir()
        .and_then(|dir| std::fs::read_to_string(dir.join(DICTIONARY_FILE)).ok())
        .map(|text| {
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

pub fn add_user_word(word: &str) {
    let Some(dir) = app_dir() else {
        return;
    };
    let mut words = user_words();
    if words.iter().any(|known| known == word) {
        return;
    }
    words.push(word.to_owned());
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(DICTIONARY_FILE), words.join("\n") + "\n");
}

pub fn size_text(bytes: u64) -> String {
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        format!(
            "{} KB",
            crate::app::locale().format_number((kb * 10.0).round() / 10.0)
        )
    } else {
        let mb = (kb / 1024.0 * 10.0).round() / 10.0;
        format!("{} MB", crate::app::locale().format_number(mb))
    }
}

pub fn reveal(path: &Path) {
    #[cfg(target_arch = "wasm32")]
    let _ = path;
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();
    #[cfg(all(unix, not(target_arch = "wasm32")))]
    if let Some(dir) = path.parent() {
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
}
