use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::text;

const FAMILY_DIR: &str = "Vellum";
const SETTINGS_FILE: &str = "settings.txt";
const RECENT_FILE: &str = "recent.txt";
const RECENT_LIMIT: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    fn key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Canvas {
    #[default]
    Dark,
    White,
}

impl Canvas {
    pub const ALL: [Self; 2] = [Self::Dark, Self::White];

    fn key(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::White => "white",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Turkish,
    English,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::Turkish, Self::English];

    fn key(self) -> &'static str {
        match self {
            Self::Turkish => "tr",
            Self::English => "en",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Turkish => "Türkçe",
            Self::English => "English",
        }
    }

    fn system() -> Self {
        let tag = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()));
        match tag {
            Some(tag) if !tag.to_ascii_lowercase().starts_with("tr") => Self::English,
            _ => Self::Turkish,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prefs {
    pub theme: ThemeChoice,
    pub canvas: Canvas,
    pub language: Language,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::System,
            canvas: Canvas::Dark,
            language: Language::system(),
        }
    }
}

impl Prefs {
    pub fn parse(text: &str) -> Self {
        let mut prefs = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "theme" => {
                    if let Some(theme) = ThemeChoice::ALL
                        .into_iter()
                        .find(|t| t.key() == value.trim())
                    {
                        prefs.theme = theme;
                    }
                }
                "canvas" => {
                    if let Some(canvas) = Canvas::ALL.into_iter().find(|c| c.key() == value.trim())
                    {
                        prefs.canvas = canvas;
                    }
                }
                "language" => {
                    if let Some(language) =
                        Language::ALL.into_iter().find(|l| l.key() == value.trim())
                    {
                        prefs.language = language;
                    }
                }
                _ => {}
            }
        }
        prefs
    }

    pub fn write(&self) -> String {
        format!(
            "theme={}\ncanvas={}\nlanguage={}\n",
            self.theme.key(),
            self.canvas.key(),
            self.language.key()
        )
    }

    pub fn load() -> Self {
        family_dir()
            .and_then(|dir| std::fs::read_to_string(dir.join(SETTINGS_FILE)).ok())
            .map(|text| Self::parse(&text))
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(dir) = family_dir() {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join(SETTINGS_FILE), self.write());
        }
    }

    pub fn is_dark(&self, system_dark: bool) -> bool {
        match self.theme {
            ThemeChoice::System => system_dark,
            ThemeChoice::Light => false,
            ThemeChoice::Dark => true,
        }
    }

    pub fn apply(&self, system_dark: bool) {
        crate::set_english(self.language == Language::English);
        let dark = self.is_dark(system_dark);
        crate::set_dark(dark);
        crate::set_dark_canvas(dark && self.canvas == Canvas::Dark);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
}

#[cfg(target_arch = "wasm32")]
fn data_dir() -> Option<PathBuf> {
    None
}

pub fn family_dir() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(FAMILY_DIR))
}

pub fn app_dir(app: &str) -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(app))
}

pub fn recent(app: &str) -> Vec<PathBuf> {
    app_dir(app)
        .and_then(|dir| std::fs::read_to_string(dir.join(RECENT_FILE)).ok())
        .map(|text| {
            text.lines()
                .filter(|line| !line.trim().is_empty())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

pub fn remember(app: &str, path: &Path) {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut list = recent(app);
    list.retain(|other| other != &path);
    list.insert(0, path);
    list.truncate(RECENT_LIMIT);
    store(app, &list);
}

pub fn forget(app: &str, path: &Path) {
    let mut list = recent(app);
    list.retain(|other| other != path);
    store(app, &list);
}

fn store(app: &str, list: &[PathBuf]) {
    let Some(dir) = app_dir(app) else {
        return;
    };
    let text: String = list
        .iter()
        .map(|path| format!("{}\n", path.display()))
        .collect();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(RECENT_FILE), text);
}

pub fn ago(time: SystemTime) -> String {
    let minutes = time.elapsed().map_or(0, |d| d.as_secs() / 60);
    let (count, one, many): (u64, text::Label, text::Label) = match minutes {
        0 => return text::just_now().to_owned(),
        1..60 => (minutes, text::minute_ago, text::minutes_ago),
        60..1440 => (minutes / 60, text::hour_ago, text::hours_ago),
        _ => (minutes / 1440, text::day_ago, text::days_ago),
    };
    let unit = if count == 1 { one() } else { many() };
    format!("{count} {unit}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefs_round_trip() {
        let prefs = Prefs {
            theme: ThemeChoice::Dark,
            canvas: Canvas::White,
            language: Language::English,
        };
        assert_eq!(Prefs::parse(&prefs.write()), prefs);
        let partial = Prefs::parse("language=tr\nunknown=1\n");
        assert_eq!(partial.language, Language::Turkish);
        assert_eq!(partial.theme, ThemeChoice::System);
        assert_eq!(partial.canvas, Canvas::Dark);
    }
}
