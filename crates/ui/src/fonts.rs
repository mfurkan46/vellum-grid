use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use eframe::egui::{Context, FontData, FontDefinitions, FontFamily, FontId};

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

struct Family {
    name: &'static str,
    files: [&'static str; 4],
}

#[cfg(target_os = "windows")]
const DIRECTORIES: &[&str] = &["C:/Windows/Fonts"];
#[cfg(not(target_os = "windows"))]
const DIRECTORIES: &[&str] = &[
    "/usr/share/fonts/truetype/crosextra",
    "/usr/share/fonts/crosextra",
    "/usr/share/fonts/truetype/liberation",
    "/usr/share/fonts/truetype/liberation2",
    "/usr/share/fonts/liberation",
    "/usr/share/fonts/truetype/dejavu",
    "/usr/share/fonts/TTF",
];

#[cfg(target_os = "windows")]
const FAMILIES: &[Family] = &[
    Family {
        name: "Calibri",
        files: [
            "calibri.ttf",
            "calibrib.ttf",
            "calibrii.ttf",
            "calibriz.ttf",
        ],
    },
    Family {
        name: "Arial",
        files: ["arial.ttf", "arialbd.ttf", "ariali.ttf", "arialbi.ttf"],
    },
    Family {
        name: "Times New Roman",
        files: ["times.ttf", "timesbd.ttf", "timesi.ttf", "timesbi.ttf"],
    },
    Family {
        name: "Courier New",
        files: ["cour.ttf", "courbd.ttf", "couri.ttf", "courbi.ttf"],
    },
    Family {
        name: "Verdana",
        files: [
            "verdana.ttf",
            "verdanab.ttf",
            "verdanai.ttf",
            "verdanaz.ttf",
        ],
    },
    Family {
        name: "Tahoma",
        files: ["tahoma.ttf", "tahomabd.ttf", "", ""],
    },
    Family {
        name: "Segoe UI",
        files: [
            "segoeui.ttf",
            "segoeuib.ttf",
            "segoeuii.ttf",
            "segoeuiz.ttf",
        ],
    },
];

#[cfg(not(target_os = "windows"))]
const FAMILIES: &[Family] = &[
    Family {
        name: "Calibri",
        files: [
            "Carlito-Regular.ttf",
            "Carlito-Bold.ttf",
            "Carlito-Italic.ttf",
            "Carlito-BoldItalic.ttf",
        ],
    },
    Family {
        name: "Arial",
        files: [
            "LiberationSans-Regular.ttf",
            "LiberationSans-Bold.ttf",
            "LiberationSans-Italic.ttf",
            "LiberationSans-BoldItalic.ttf",
        ],
    },
    Family {
        name: "Times New Roman",
        files: [
            "LiberationSerif-Regular.ttf",
            "LiberationSerif-Bold.ttf",
            "LiberationSerif-Italic.ttf",
            "LiberationSerif-BoldItalic.ttf",
        ],
    },
    Family {
        name: "Courier New",
        files: [
            "LiberationMono-Regular.ttf",
            "LiberationMono-Bold.ttf",
            "LiberationMono-Italic.ttf",
            "LiberationMono-BoldItalic.ttf",
        ],
    },
    Family {
        name: "Verdana",
        files: [
            "DejaVuSans.ttf",
            "DejaVuSans-Bold.ttf",
            "DejaVuSans-Oblique.ttf",
            "DejaVuSans-BoldOblique.ttf",
        ],
    },
];

static LOADED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn read(file: &str) -> Option<Vec<u8>> {
    if file.is_empty() {
        return None;
    }
    DIRECTORIES
        .iter()
        .find_map(|dir| std::fs::read(format!("{dir}/{file}")).ok())
}

fn variant_name(family: &str, bold: bool, italic: bool) -> String {
    match (bold, italic) {
        (false, false) => family.to_owned(),
        (true, false) => format!("{family}:b"),
        (false, true) => format!("{family}:i"),
        (true, true) => format!("{family}:bi"),
    }
}

pub fn install(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut loaded = HashSet::new();
    for family in FAMILIES {
        for (index, file) in family.files.iter().enumerate() {
            let Some(bytes) = read(file) else {
                continue;
            };
            let name = variant_name(family.name, index % 2 == 1, index >= 2);
            fonts
                .font_data
                .insert(name.clone(), Arc::new(FontData::from_owned(bytes)));
            let mut chain = vec![name.clone()];
            chain.extend(fallback.iter().cloned());
            fonts
                .families
                .insert(FontFamily::Name(name.as_str().into()), chain);
            loaded.insert(name);
        }
    }
    for (family, bold, italic) in [
        (CHROME, false, false),
        (CHROME_BOLD, true, false),
        (CHROME_ITALIC, false, true),
    ] {
        let first = ["Segoe UI", "Verdana", DEFAULT_FONT]
            .into_iter()
            .map(|name| variant_name(name, bold, italic))
            .find(|name| loaded.contains(name));
        let mut chain: Vec<String> = first.into_iter().collect();
        chain.extend(fallback.iter().cloned());
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
    if !loaded.contains(DEFAULT_FONT) {
        fonts
            .families
            .insert(FontFamily::Name(DEFAULT_FONT.into()), fallback);
    }
    *LOADED.lock().expect("font registry lock") = Some(loaded);
    ctx.set_fonts(fonts);
}

pub fn font_file(family: &str, bold: bool, italic: bool) -> Option<Vec<u8>> {
    let family = FAMILIES
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case(family))?;
    let index = usize::from(bold) + 2 * usize::from(italic);
    read(family.files[index])
}

pub struct DocumentFont {
    pub id: FontId,
    pub faux_bold: bool,
    pub faux_italic: bool,
}

pub fn document_font(family: &str, bold: bool, italic: bool, size: f32) -> DocumentFont {
    let loaded = LOADED.lock().expect("font registry lock");
    let has = |name: &str| loaded.as_ref().is_some_and(|l| l.contains(name));
    let family = if has(family) { family } else { DEFAULT_FONT };
    let wanted = variant_name(family, bold, italic);
    let (name, faux_bold, faux_italic) = if has(&wanted) {
        (wanted, false, false)
    } else if bold && has(&variant_name(family, true, false)) {
        (variant_name(family, true, false), false, italic)
    } else if italic && has(&variant_name(family, false, true)) {
        (variant_name(family, false, true), bold, false)
    } else {
        (family.to_owned(), bold, italic)
    };
    DocumentFont {
        id: FontId::new(size, FontFamily::Name(name.as_str().into())),
        faux_bold,
        faux_italic,
    }
}

pub fn family_for(family: &str, bold: bool, italic: bool) -> FontFamily {
    let loaded = LOADED.lock().expect("font registry lock");
    let has = |name: &str| loaded.as_ref().is_some_and(|l| l.contains(name));
    let base = if has(family) { family } else { DEFAULT_FONT };
    let wanted = variant_name(base, bold, italic);
    let name = if has(&wanted) {
        wanted
    } else {
        base.to_owned()
    };
    FontFamily::Name(name.as_str().into())
}

pub fn available() -> Vec<&'static str> {
    let loaded = LOADED.lock().expect("font registry lock");
    FAMILIES
        .iter()
        .map(|f| f.name)
        .filter(|name| loaded.as_ref().is_some_and(|l| l.contains(*name)))
        .collect()
}

pub fn px(points: f32) -> f32 {
    points * 96.0 / 72.0
}
