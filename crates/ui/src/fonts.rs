use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use eframe::egui::{
    Context, FontData, FontDefinitions, FontFamily, FontId, FontInsert, FontPriority, FontProvider,
    InsertFontFamily,
};

pub const DEFAULT_FONT: &str = "Calibri";
pub const CHROME: &str = "chrome";
pub const CHROME_BOLD: &str = "chrome:b";
pub const CHROME_ITALIC: &str = "chrome:i";
pub const FLUENT: &str = "fluent";
pub const FLUENT_FILLED: &str = "fluent:filled";

const FLUENT_FONTS: [(&str, &[u8]); 2] = [
    (
        FLUENT,
        include_bytes!("../../../assets/fonts/fluent-regular.ttf"),
    ),
    (
        FLUENT_FILLED,
        include_bytes!("../../../assets/fonts/fluent-filled.ttf"),
    ),
];

const CHROME_FAMILIES: [&str; 3] = ["Segoe UI", "Verdana", DEFAULT_FONT];

const ALIASES: [(&str, &str); 6] = [
    ("Calibri", "Carlito"),
    ("Cambria", "Caladea"),
    ("Arial", "Liberation Sans"),
    ("Times New Roman", "Liberation Serif"),
    ("Courier New", "Liberation Mono"),
    ("Verdana", "DejaVu Sans"),
];

const EXTENSIONS: [&str; 4] = ["ttf", "otf", "ttc", "otc"];
const PREVIEW_LIMIT: u64 = 3 * 1024 * 1024;

#[derive(Debug, Clone)]
struct Source {
    path: PathBuf,
    index: u32,
    collection: bool,
    size: u64,
    symbol: bool,
}

#[derive(Debug, Clone)]
struct Family {
    name: String,
    faces: [Option<Source>; 4],
}

#[derive(Debug, Default)]
struct Catalog {
    families: Vec<Family>,
    exact: HashMap<String, usize>,
    folded: HashMap<String, usize>,
}

impl Catalog {
    fn scan(directories: &[PathBuf]) -> Self {
        let mut files = Vec::new();
        for directory in directories {
            collect(directory, &mut files);
        }
        let mut catalog = Self::default();
        for (path, size) in files {
            for face in read_faces(&path) {
                let source = Source {
                    path: path.clone(),
                    index: face.index,
                    collection: face.collection,
                    size,
                    symbol: face.symbol,
                };
                catalog.add(&face.family, face.bold, face.italic, source);
            }
        }
        catalog.add_aliases();
        catalog
            .families
            .sort_by_key(|family| family.name.to_lowercase());
        catalog.reindex();
        catalog
    }

    fn add(&mut self, name: &str, bold: bool, italic: bool, source: Source) {
        let slot = usize::from(bold) + 2 * usize::from(italic);
        let index = match self.find(name) {
            Some(index) => index,
            None => self.insert(Family {
                name: name.to_owned(),
                faces: Default::default(),
            }),
        };
        self.families[index].faces[slot].get_or_insert(source);
    }

    fn add_aliases(&mut self) {
        for (alias, target) in ALIASES {
            if self.find(alias).is_none()
                && let Some(index) = self.find(target)
            {
                let faces = self.families[index].faces.clone();
                self.insert(Family {
                    name: alias.to_owned(),
                    faces,
                });
            }
        }
    }

    fn insert(&mut self, family: Family) -> usize {
        let index = self.families.len();
        self.exact.insert(family.name.clone(), index);
        self.folded.insert(family.name.to_lowercase(), index);
        self.families.push(family);
        index
    }

    fn reindex(&mut self) {
        self.exact.clear();
        self.folded.clear();
        for (index, family) in self.families.iter().enumerate() {
            self.exact.insert(family.name.clone(), index);
            self.folded.insert(family.name.to_lowercase(), index);
        }
    }

    fn find(&self, name: &str) -> Option<usize> {
        self.exact
            .get(name)
            .or_else(|| self.folded.get(&name.to_lowercase()))
            .copied()
    }

    fn family(&self, name: &str) -> Option<&Family> {
        self.find(name).map(|index| &self.families[index])
    }

    fn source(&self, name: &str, bold: bool, italic: bool) -> Option<&Source> {
        self.family(name)?.faces[usize::from(bold) + 2 * usize::from(italic)].as_ref()
    }
}

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| Catalog::scan(&directories()))
}

/// Starts reading the installed fonts in the background, so that the list is ready by the
/// time the window opens.
pub fn prefetch() {
    std::thread::spawn(|| {
        catalog();
    });
}

#[cfg(target_os = "windows")]
fn directories() -> Vec<PathBuf> {
    let windows =
        std::env::var_os("WINDIR").map_or_else(|| PathBuf::from("C:/Windows"), PathBuf::from);
    let mut out = vec![windows.join("Fonts")];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        out.push(PathBuf::from(local).join("Microsoft/Windows/Fonts"));
    }
    out
}

#[cfg(not(target_os = "windows"))]
fn directories() -> Vec<PathBuf> {
    let mut out = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match std::env::var_os("XDG_DATA_HOME") {
        Some(data) => out.push(PathBuf::from(data).join("fonts")),
        None => out.extend(home.iter().map(|home| home.join(".local/share/fonts"))),
    }
    out.extend(home.iter().map(|home| home.join(".fonts")));
    out
}

fn collect(directory: &Path, out: &mut Vec<(PathBuf, u64)>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            collect(&path, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
        {
            out.push((path, meta.len()));
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Face {
    family: String,
    bold: bool,
    italic: bool,
    index: u32,
    collection: bool,
    symbol: bool,
}

fn read_at(file: &mut File, offset: u64, len: usize) -> Option<Vec<u8>> {
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut buf = vec![0; len];
    file.read_exact(&mut buf).ok()?;
    Some(buf)
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn read_faces(path: &Path) -> Vec<Face> {
    let Ok(mut file) = File::open(path) else {
        return Vec::new();
    };
    let Some(header) = read_at(&mut file, 0, 12) else {
        return Vec::new();
    };
    let collection = &header[..4] == b"ttcf";
    let offsets: Vec<u32> = if collection {
        let count = u32_at(&header, 8).unwrap_or(0).min(256) as usize;
        read_at(&mut file, 12, count * 4)
            .map(|table| (0..count).filter_map(|i| u32_at(&table, i * 4)).collect())
            .unwrap_or_default()
    } else {
        vec![0]
    };
    offsets
        .into_iter()
        .zip(0u32..)
        .filter_map(|(offset, index)| {
            let (family, bold, italic, symbol) = read_face(&mut file, u64::from(offset))?;
            Some(Face {
                family,
                bold,
                italic,
                index,
                collection,
                symbol,
            })
        })
        .collect()
}

fn read_face(file: &mut File, offset: u64) -> Option<(String, bool, bool, bool)> {
    let directory = read_at(file, offset, 12)?;
    let count = usize::from(u16_at(&directory, 4)?);
    let records = read_at(file, offset + 12, count * 16)?;
    let table = |tag: &[u8; 4]| {
        (0..count).find_map(|i| {
            let record = &records[i * 16..i * 16 + 16];
            (&record[..4] == tag).then(|| (u32_at(record, 8), u32_at(record, 12)))
        })
    };
    let (name_offset, name_len) = table(b"name")?;
    let names = read_at(file, u64::from(name_offset?), name_len? as usize)?;
    let family = name_string(&names, 1)?;
    if family.is_empty() || family.starts_with('.') {
        return None;
    }
    match table(b"OS/2") {
        Some((Some(offset), Some(len))) if len >= 64 => {
            let os2 = read_at(file, u64::from(offset), len.min(82) as usize)?;
            let flags = u16_at(&os2, 62)?;
            let symbol = u32_at(&os2, 78).is_some_and(|pages| pages & 0x8000_0000 != 0);
            Some((family, flags & 0x20 != 0, flags & 0x01 != 0, symbol))
        }
        _ => {
            let (offset, _) = table(b"head")?;
            let head = read_at(file, u64::from(offset?), 46)?;
            let flags = u16_at(&head, 44)?;
            Some((family, flags & 0x01 != 0, flags & 0x02 != 0, false))
        }
    }
}

fn name_string(table: &[u8], id: u16) -> Option<String> {
    let count = usize::from(u16_at(table, 2)?);
    let strings = usize::from(u16_at(table, 4)?);
    let mut best: Option<(u8, &[u8], u16)> = None;
    for i in 0..count {
        let at = 6 + i * 12;
        let platform = u16_at(table, at)?;
        let encoding = u16_at(table, at + 2)?;
        let language = u16_at(table, at + 4)?;
        if u16_at(table, at + 6)? != id {
            continue;
        }
        let len = usize::from(u16_at(table, at + 8)?);
        let start = strings + usize::from(u16_at(table, at + 10)?);
        let Some(bytes) = table.get(start..start + len) else {
            continue;
        };
        let score = match (platform, encoding, language) {
            (3, 1 | 10, 0x409) => 4,
            (3, 1 | 10, _) => 3,
            (0, _, _) => 2,
            (1, 0, 0) => 1,
            _ => continue,
        };
        if best.is_none_or(|(s, _, _)| score > s) {
            best = Some((score, bytes, platform));
        }
    }
    let (_, bytes, platform) = best?;
    let text = if platform == 1 {
        bytes.iter().map(|&b| char::from(b)).collect()
    } else {
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_be_bytes(*pair))
            .collect();
        String::from_utf16_lossy(&units)
    };
    Some(text.trim().to_owned())
}

fn variant_name(family: &str, bold: bool, italic: bool) -> String {
    match (bold, italic) {
        (false, false) => family.to_owned(),
        (true, false) => format!("{family}:b"),
        (false, true) => format!("{family}:i"),
        (true, true) => format!("{family}:bi"),
    }
}

fn split_variant(name: &str) -> (&str, bool, bool) {
    match name.rsplit_once(':') {
        Some((family, "b")) => (family, true, false),
        Some((family, "i")) => (family, false, true),
        Some((family, "bi")) => (family, true, true),
        _ => (name, false, false),
    }
}

fn load(source: &Source) -> Option<FontData> {
    let bytes = std::fs::read(&source.path).ok()?;
    Some(FontData::from_blob(Arc::new(bytes), source.index))
}

struct SystemFonts {
    fallback: Vec<FontInsert>,
}

impl FontProvider for SystemFonts {
    fn fonts_for_family(&self, family: &FontFamily) -> Vec<FontInsert> {
        let FontFamily::Name(name) = family else {
            return Vec::new();
        };
        let (base, bold, italic) = split_variant(name);
        let mut out: Vec<FontInsert> = catalog()
            .source(base, bold, italic)
            .and_then(load)
            .map(|data| FontInsert::new(name, data, Vec::new()))
            .into_iter()
            .collect();
        out.extend(self.fallback.iter().cloned());
        out
    }
}

pub fn install(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    let fallback_names = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let fallback: Vec<FontInsert> = fallback_names
        .iter()
        .filter_map(|name| {
            let data = fonts.font_data.get(name)?;
            Some(FontInsert::new(
                name,
                (**data).clone(),
                vec![InsertFontFamily {
                    family: FontFamily::Proportional,
                    priority: FontPriority::Lowest,
                }],
            ))
        })
        .collect();
    for (family, bold, italic) in [
        (CHROME, false, false),
        (CHROME_BOLD, true, false),
        (CHROME_ITALIC, false, true),
    ] {
        let first = CHROME_FAMILIES.into_iter().find_map(|name| {
            let source = catalog().source(name, bold, italic)?;
            Some((variant_name(name, bold, italic), load(source)?))
        });
        let mut chain = Vec::new();
        if let Some((name, data)) = first {
            fonts.font_data.insert(name.clone(), Arc::new(data));
            chain.push(name);
        }
        chain.extend(fallback_names.iter().cloned());
        fonts
            .families
            .insert(FontFamily::Name(family.into()), chain);
    }
    for (name, bytes) in FLUENT_FONTS {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
        fonts
            .families
            .insert(FontFamily::Name(name.into()), vec![name.to_owned()]);
    }
    ctx.set_fonts(fonts);
    ctx.add_font_provider(Arc::new(SystemFonts { fallback }));
}

pub fn font_file(family: &str, bold: bool, italic: bool) -> Option<Vec<u8>> {
    let source = catalog().source(family, bold, italic)?;
    if source.collection {
        return None;
    }
    std::fs::read(&source.path).ok()
}

pub struct DocumentFont {
    pub id: FontId,
    pub faux_bold: bool,
    pub faux_italic: bool,
}

fn resolve(family: &str) -> &'static Family {
    let catalog = catalog();
    catalog
        .family(family)
        .or_else(|| catalog.family(DEFAULT_FONT))
        .unwrap_or_else(|| {
            static EMPTY: OnceLock<Family> = OnceLock::new();
            EMPTY.get_or_init(|| Family {
                name: DEFAULT_FONT.to_owned(),
                faces: Default::default(),
            })
        })
}

pub fn document_font(family: &str, bold: bool, italic: bool, size: f32) -> DocumentFont {
    let family = resolve(family);
    let has = |bold: bool, italic: bool| {
        family.faces[usize::from(bold) + 2 * usize::from(italic)].is_some()
    };
    let (bold_face, italic_face, faux_bold, faux_italic) = if has(bold, italic) {
        (bold, italic, false, false)
    } else if bold && has(true, false) {
        (true, false, false, italic)
    } else if italic && has(false, true) {
        (false, true, bold, false)
    } else {
        (false, false, bold, italic)
    };
    let name = variant_name(&family.name, bold_face, italic_face);
    DocumentFont {
        id: FontId::new(size, FontFamily::Name(name.as_str().into())),
        faux_bold,
        faux_italic,
    }
}

pub fn family_for(family: &str, bold: bool, italic: bool) -> FontFamily {
    let family = resolve(family);
    let slot = usize::from(bold) + 2 * usize::from(italic);
    let name = if family.faces[slot].is_some() {
        variant_name(&family.name, bold, italic)
    } else {
        family.name.clone()
    };
    FontFamily::Name(name.as_str().into())
}

/// Every installed font family, sorted by name, for font menus.
pub fn available() -> Vec<&'static str> {
    catalog()
        .families
        .iter()
        .filter(|family| family.faces.iter().any(Option::is_some))
        .map(|family| family.name.as_str())
        .collect()
}

/// Whether a font menu may draw the family's name in the family itself. Symbol fonts and
/// very large files (mostly East Asian fonts) are listed in the interface font instead.
pub fn previewable(family: &str) -> bool {
    catalog()
        .family(family)
        .and_then(|family| family.faces.iter().flatten().next())
        .is_some_and(|source| !source.symbol && source.size <= PREVIEW_LIMIT)
}

pub fn px(points: f32) -> f32 {
    points * 96.0 / 72.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_table(records: &[(u16, u16, u16, u16, &[u8])]) -> Vec<u8> {
        let mut head = vec![0, 0];
        head.extend((records.len() as u16).to_be_bytes());
        head.extend(((6 + records.len() * 12) as u16).to_be_bytes());
        let mut strings = Vec::new();
        for (platform, encoding, language, id, bytes) in records {
            for value in [*platform, *encoding, *language, *id, bytes.len() as u16] {
                head.extend(value.to_be_bytes());
            }
            head.extend((strings.len() as u16).to_be_bytes());
            strings.extend_from_slice(bytes);
        }
        head.extend(strings);
        head
    }

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_be_bytes).collect()
    }

    #[test]
    fn reads_the_english_family_name() {
        let turkish = utf16("Yazı");
        let english = utf16("Segoe UI Semibold");
        let table = name_table(&[
            (1, 0, 0, 1, b"Mac Name"),
            (3, 1, 0x41F, 1, &turkish),
            (3, 1, 0x409, 1, &english),
            (3, 1, 0x409, 2, &utf16("Regular")),
        ]);
        assert_eq!(name_string(&table, 1).as_deref(), Some("Segoe UI Semibold"));
        assert_eq!(name_string(&table, 2).as_deref(), Some("Regular"));
        assert_eq!(name_string(&table, 4), None);
    }

    #[test]
    fn variant_names_round_trip() {
        for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
            let name = variant_name("Times New Roman", bold, italic);
            assert_eq!(split_variant(&name), ("Times New Roman", bold, italic));
        }
    }

    #[test]
    fn aliases_fill_missing_families() {
        let source = |index| Source {
            path: PathBuf::new(),
            index,
            collection: false,
            size: 0,
            symbol: false,
        };
        let mut catalog = Catalog::default();
        catalog.add("Carlito", false, false, source(0));
        catalog.add("Carlito", true, false, source(1));
        catalog.add("Zeta", false, false, source(2));
        catalog.add_aliases();
        assert_eq!(
            catalog.source("calibri", true, false).map(|s| s.index),
            Some(1)
        );
        assert!(catalog.source("Calibri", false, true).is_none());
        assert!(catalog.find("Arial").is_none());
    }

    #[test]
    fn scans_the_installed_fonts() {
        let catalog = Catalog::scan(&directories());
        let names: Vec<&str> = catalog.families.iter().map(|f| f.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_by_key(|name| name.to_lowercase());
        assert_eq!(names, sorted);
    }
}
