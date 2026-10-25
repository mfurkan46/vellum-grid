use std::sync::LazyLock;

pub const GENERAL: &str = "General";
pub const DEFAULT_FONT: &str = "Calibri";
pub const DEFAULT_SIZE: u16 = 110;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color(pub u32);

impl Color {
    pub const BLACK: Self = Self(0x000000);
    pub const WHITE: Self = Self(0xFFFFFF);

    pub fn rgb(self) -> [u8; 3] {
        let [_, r, g, b] = self.0.to_be_bytes();
        [r, g, b]
    }

    pub fn hex(self) -> String {
        format!("{:06X}", self.0)
    }

    pub fn tinted(self, tint: f64) -> Self {
        let channel = |c: u8| {
            let c = f64::from(c);
            let tinted = if tint < 0.0 {
                c * (1.0 + tint)
            } else {
                c + (255.0 - c) * tint
            };
            tinted.round().clamp(0.0, 255.0) as u32
        };
        let [r, g, b] = self.rgb();
        Self((channel(r) << 16) | (channel(g) << 8) | channel(b))
    }

    pub fn from_hex(text: &str) -> Option<Self> {
        let digits = text.trim_start_matches('#');
        let digits = if digits.len() == 8 {
            &digits[2..]
        } else {
            digits
        };
        (digits.len() == 6)
            .then(|| u32::from_str_radix(digits, 16).ok().map(Self))
            .flatten()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HAlign {
    #[default]
    General,
    Left,
    Center,
    Right,
    Fill,
    Justify,
    CenterAcross,
    Distributed,
}

impl HAlign {
    pub const ALL: [Self; 8] = [
        Self::General,
        Self::Left,
        Self::Center,
        Self::Right,
        Self::Fill,
        Self::Justify,
        Self::CenterAcross,
        Self::Distributed,
    ];

    pub fn indents(self) -> bool {
        matches!(self, Self::Left | Self::Right | Self::Distributed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VAlign {
    Top,
    Center,
    #[default]
    Bottom,
    Justify,
    Distributed,
}

impl VAlign {
    pub const ALL: [Self; 5] = [
        Self::Top,
        Self::Center,
        Self::Bottom,
        Self::Justify,
        Self::Distributed,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineStyle {
    Hair,
    Dotted,
    DashDotDot,
    DashDot,
    Dashed,
    Thin,
    MediumDashDotDot,
    SlantDashDot,
    MediumDashDot,
    MediumDashed,
    Medium,
    Thick,
    Double,
}

impl LineStyle {
    pub const ALL: [Self; 13] = [
        Self::Hair,
        Self::Dotted,
        Self::DashDotDot,
        Self::DashDot,
        Self::Dashed,
        Self::Thin,
        Self::MediumDashDotDot,
        Self::SlantDashDot,
        Self::MediumDashDot,
        Self::MediumDashed,
        Self::Medium,
        Self::Thick,
        Self::Double,
    ];

    pub fn weight(self) -> u8 {
        match self {
            Self::Thick | Self::Double => 3,
            Self::MediumDashDotDot
            | Self::SlantDashDot
            | Self::MediumDashDot
            | Self::MediumDashed
            | Self::Medium => 2,
            _ => 1,
        }
    }

    pub fn dashes(self) -> &'static [f32] {
        match self {
            Self::Hair => &[1.0, 1.0],
            Self::Dotted => &[1.0, 2.0],
            Self::DashDotDot | Self::MediumDashDotDot => &[6.0, 2.0, 2.0, 2.0, 2.0, 2.0],
            Self::DashDot | Self::MediumDashDot => &[6.0, 2.0, 2.0, 2.0],
            Self::Dashed => &[3.0, 2.0],
            Self::MediumDashed => &[6.0, 2.0],
            Self::SlantDashDot => &[8.0, 1.0, 3.0, 1.0],
            Self::Thin | Self::Medium | Self::Thick | Self::Double => &[],
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Hair => "hair",
            Self::Dotted => "dotted",
            Self::DashDotDot => "dashDotDot",
            Self::DashDot => "dashDot",
            Self::Dashed => "dashed",
            Self::Thin => "thin",
            Self::MediumDashDotDot => "mediumDashDotDot",
            Self::SlantDashDot => "slantDashDot",
            Self::MediumDashDot => "mediumDashDot",
            Self::MediumDashed => "mediumDashed",
            Self::Medium => "medium",
            Self::Thick => "thick",
            Self::Double => "double",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|style| style.key() == key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Underline {
    #[default]
    None,
    Single,
    Double,
    SingleAccounting,
    DoubleAccounting,
}

impl Underline {
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::Single,
        Self::Double,
        Self::SingleAccounting,
        Self::DoubleAccounting,
    ];

    pub fn is_double(self) -> bool {
        matches!(self, Self::Double | Self::DoubleAccounting)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Script {
    #[default]
    None,
    Super,
    Sub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatternKind {
    Gray75,
    Gray50,
    Gray25,
    Gray125,
    Gray0625,
    DarkHorizontal,
    DarkVertical,
    DarkDown,
    DarkUp,
    DarkGrid,
    DarkTrellis,
    LightHorizontal,
    LightVertical,
    LightDown,
    LightUp,
    LightGrid,
    LightTrellis,
}

impl PatternKind {
    pub const ALL: [Self; 17] = [
        Self::Gray75,
        Self::Gray50,
        Self::Gray25,
        Self::Gray125,
        Self::Gray0625,
        Self::DarkHorizontal,
        Self::DarkVertical,
        Self::DarkDown,
        Self::DarkUp,
        Self::DarkGrid,
        Self::DarkTrellis,
        Self::LightHorizontal,
        Self::LightVertical,
        Self::LightDown,
        Self::LightUp,
        Self::LightGrid,
        Self::LightTrellis,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Gray75 => "darkGray",
            Self::Gray50 => "mediumGray",
            Self::Gray25 => "lightGray",
            Self::Gray125 => "gray125",
            Self::Gray0625 => "gray0625",
            Self::DarkHorizontal => "darkHorizontal",
            Self::DarkVertical => "darkVertical",
            Self::DarkDown => "darkDown",
            Self::DarkUp => "darkUp",
            Self::DarkGrid => "darkGrid",
            Self::DarkTrellis => "darkTrellis",
            Self::LightHorizontal => "lightHorizontal",
            Self::LightVertical => "lightVertical",
            Self::LightDown => "lightDown",
            Self::LightUp => "lightUp",
            Self::LightGrid => "lightGrid",
            Self::LightTrellis => "lightTrellis",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == key)
    }

    pub fn tile(self) -> [u8; 8] {
        match self {
            Self::Gray75 => [0x77, 0xDD, 0x77, 0xDD, 0x77, 0xDD, 0x77, 0xDD],
            Self::Gray50 => [0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA],
            Self::Gray25 => [0x88, 0x22, 0x88, 0x22, 0x88, 0x22, 0x88, 0x22],
            Self::Gray125 => [0x80, 0x00, 0x08, 0x00, 0x80, 0x00, 0x08, 0x00],
            Self::Gray0625 => [0x80, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00],
            Self::DarkHorizontal => [0xFF, 0xFF, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00],
            Self::DarkVertical => [0xCC; 8],
            Self::DarkDown => [0xCC, 0x66, 0x33, 0x99, 0xCC, 0x66, 0x33, 0x99],
            Self::DarkUp => [0x33, 0x66, 0xCC, 0x99, 0x33, 0x66, 0xCC, 0x99],
            Self::DarkGrid => [0xFF, 0xFF, 0xCC, 0xCC, 0xFF, 0xFF, 0xCC, 0xCC],
            Self::DarkTrellis => [0xFF, 0x66, 0xFF, 0x99, 0xFF, 0x66, 0xFF, 0x99],
            Self::LightHorizontal => [0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00],
            Self::LightVertical => [0x88; 8],
            Self::LightDown => [0x88, 0x44, 0x22, 0x11, 0x88, 0x44, 0x22, 0x11],
            Self::LightUp => [0x11, 0x22, 0x44, 0x88, 0x11, 0x22, 0x44, 0x88],
            Self::LightGrid => [0xFF, 0x88, 0x88, 0x88, 0xFF, 0x88, 0x88, 0x88],
            Self::LightTrellis => [0x99, 0x66, 0x99, 0x66, 0x99, 0x66, 0x99, 0x66],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pattern {
    pub kind: PatternKind,
    pub color: Color,
}

pub const STACKED: i16 = 255;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Border {
    pub style: LineStyle,
    pub color: Color,
}

impl Border {
    pub fn thin() -> Self {
        Self {
            style: LineStyle::Thin,
            color: Color::BLACK,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Borders {
    pub top: Option<Border>,
    pub right: Option<Border>,
    pub bottom: Option<Border>,
    pub left: Option<Border>,
    pub diagonal: Option<Border>,
    pub diagonal_up: bool,
    pub diagonal_down: bool,
}

impl Borders {
    pub fn diagonals(&self) -> impl Iterator<Item = (Border, bool)> {
        let up = self
            .diagonal
            .filter(|_| self.diagonal_up)
            .map(|b| (b, true));
        let down = self
            .diagonal
            .filter(|_| self.diagonal_down)
            .map(|b| (b, false));
        up.into_iter().chain(down)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Style {
    pub number_format: String,
    pub font: String,
    pub size: u16,
    pub bold: bool,
    pub italic: bool,
    pub underline: Underline,
    pub strike: bool,
    pub script: Script,
    pub color: Option<Color>,
    pub fill: Option<Color>,
    pub pattern: Option<Pattern>,
    pub borders: Borders,
    pub h_align: HAlign,
    pub v_align: VAlign,
    pub wrap: bool,
    pub shrink: bool,
    pub indent: u8,
    pub rotation: i16,
    pub locked: bool,
    pub hidden: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            number_format: GENERAL.to_owned(),
            font: DEFAULT_FONT.to_owned(),
            size: DEFAULT_SIZE,
            bold: false,
            italic: false,
            underline: Underline::None,
            strike: false,
            script: Script::None,
            color: None,
            fill: None,
            pattern: None,
            borders: Borders::default(),
            h_align: HAlign::General,
            v_align: VAlign::Bottom,
            wrap: false,
            shrink: false,
            indent: 0,
            rotation: 0,
            locked: true,
            hidden: false,
        }
    }
}

pub static DEFAULT_STYLE: LazyLock<Style> = LazyLock::new(Style::default);

impl Style {
    pub fn with_number_format(code: &str) -> Self {
        Self {
            number_format: code.to_owned(),
            ..Self::default()
        }
    }

    pub fn is_default(&self) -> bool {
        *self == *DEFAULT_STYLE
    }

    pub fn points(&self) -> f32 {
        f32::from(self.size) / 10.0
    }

    pub fn has_general_format(&self) -> bool {
        self.number_format.eq_ignore_ascii_case(GENERAL)
    }

    pub fn is_stacked(&self) -> bool {
        self.rotation == STACKED
    }

    pub fn angle(&self) -> i16 {
        if self.is_stacked() { 0 } else { self.rotation }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tints_lighten_and_darken() {
        assert_eq!(Color(0x4472C4).tinted(0.8), Color(0xDAE3F3));
        assert_eq!(Color(0x4472C4).tinted(-0.5), Color(0x223962));
        assert_eq!(Color::WHITE.tinted(-0.05), Color(0xF2F2F2));
        assert_eq!(Color::BLACK.tinted(0.5), Color(0x808080));
    }

    #[test]
    fn colors_convert_to_and_from_hex() {
        assert_eq!(Color::from_hex("FF0000"), Some(Color(0xFF0000)));
        assert_eq!(Color::from_hex("FF107C41"), Some(Color(0x107C41)));
        assert_eq!(Color::from_hex("#00ff00"), Some(Color(0x00FF00)));
        assert_eq!(Color::from_hex("xyz"), None);
        assert_eq!(Color(0x107C41).hex(), "107C41");
        assert_eq!(Color(0x107C41).rgb(), [0x10, 0x7C, 0x41]);
    }

    #[test]
    fn line_and_pattern_keys_round_trip() {
        for style in LineStyle::ALL {
            assert_eq!(LineStyle::from_key(style.key()), Some(style));
        }
        for kind in PatternKind::ALL {
            assert_eq!(PatternKind::from_key(kind.key()), Some(kind));
        }
        assert_eq!(LineStyle::Thick.weight(), 3);
        assert!(LineStyle::Thin.dashes().is_empty());
    }

    #[test]
    fn diagonals_follow_flags() {
        let borders = Borders {
            diagonal: Some(Border::thin()),
            diagonal_down: true,
            ..Borders::default()
        };
        let lines: Vec<bool> = borders.diagonals().map(|(_, up)| up).collect();
        assert_eq!(lines, [false]);
    }

    #[test]
    fn default_style_values() {
        let style = Style::default();
        assert!(style.is_default());
        assert!(style.has_general_format());
        assert_eq!(style.points(), 11.0);
        assert_eq!(style.v_align, VAlign::Bottom);
        assert!(!Style::with_number_format("0.00").is_default());
    }
}
