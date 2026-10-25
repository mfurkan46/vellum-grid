use crate::table::Table;
use crate::{Border, Borders, CellAddr, Color, LineStyle, Style};

const ACCENTS: [u32; 7] = [
    0x000000, 0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000, 0x5B9BD5, 0x70AD47,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Light,
    Medium,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleId {
    pub family: Family,
    pub number: u8,
}

impl StyleId {
    pub fn all() -> impl Iterator<Item = Self> {
        let family = |family, count: u8| (1..=count).map(move |number| Self { family, number });
        family(Family::Light, 21)
            .chain(family(Family::Medium, 28))
            .chain(family(Family::Dark, 11))
    }

    pub fn parse(name: &str) -> Option<Self> {
        let rest = name.strip_prefix("TableStyle")?;
        let (family, digits) = if let Some(n) = rest.strip_prefix("Light") {
            (Family::Light, n)
        } else if let Some(n) = rest.strip_prefix("Medium") {
            (Family::Medium, n)
        } else {
            (Family::Dark, rest.strip_prefix("Dark")?)
        };
        let number: u8 = digits.parse().ok()?;
        let max = match family {
            Family::Light => 21,
            Family::Medium => 28,
            Family::Dark => 11,
        };
        (1..=max)
            .contains(&number)
            .then_some(Self { family, number })
    }

    pub fn name(self) -> String {
        let family = match self.family {
            Family::Light => "Light",
            Family::Medium => "Medium",
            Family::Dark => "Dark",
        };
        format!("TableStyle{family}{}", self.number)
    }

    fn group(self) -> u8 {
        (self.number - 1) / 7
    }

    fn accent(self) -> Color {
        let index = match (self.family, self.number) {
            (Family::Dark, n @ 8..=11) => usize::from(n - 8) * 2 % 7,
            (_, n) => usize::from((n - 1) % 7),
        };
        Color(ACCENTS[index])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TableLook {
    pub fill: Option<Color>,
    pub color: Option<Color>,
    pub bold: bool,
    pub top: Option<Border>,
    pub bottom: Option<Border>,
    pub left: Option<Border>,
    pub right: Option<Border>,
}

impl TableLook {
    pub fn apply(&self, style: &mut Style) {
        style.fill = style.fill.or(self.fill);
        style.color = style.color.or(self.color);
        style.bold |= self.bold;
        let borders = &mut style.borders;
        borders.top = borders.top.or(self.top);
        borders.bottom = borders.bottom.or(self.bottom);
        borders.left = borders.left.or(self.left);
        borders.right = borders.right.or(self.right);
    }

    pub fn borders(&self) -> Borders {
        Borders {
            top: self.top,
            bottom: self.bottom,
            left: self.left,
            right: self.right,
            ..Borders::default()
        }
    }
}

fn line(style: LineStyle, color: Color) -> Option<Border> {
    Some(Border { style, color })
}

fn tint(color: Color, amount: f64) -> Color {
    if color == Color::BLACK {
        Color(0x808080).tinted(amount.max(0.0) * 0.9 + 0.1)
    } else {
        color.tinted(amount)
    }
}

pub fn table_look(table: &Table, addr: CellAddr) -> Option<TableLook> {
    if !table.range.contains(addr) {
        return None;
    }
    let id = StyleId::parse(&table.style)?;
    let accent = id.accent();
    let header = table.header_row() == Some(addr.row());
    let totals = table.totals_row() == Some(addr.row());
    let (data_top, _) = table.data_rows();
    let band_row =
        !header && !totals && addr.row() >= data_top && (addr.row() - data_top).is_multiple_of(2);
    let band_col = (addr.col() - table.range.start().col()).is_multiple_of(2);
    let banded =
        (table.banded_rows && band_row) || (table.banded_cols && !header && !totals && band_col);
    let first = table.first_col && addr.col() == table.range.start().col();
    let last = table.last_col && addr.col() == table.range.end().col();
    let top_edge = addr.row() == table.range.start().row();
    let bottom_edge = addr.row() == table.range.end().row();
    let thin = |c| line(LineStyle::Thin, c);
    let white = Some(Color::WHITE);
    let mut look = TableLook {
        bold: header || totals || first || last,
        ..TableLook::default()
    };
    match (id.family, id.group()) {
        (Family::Light, 0) => {
            if banded {
                look.fill = Some(tint(accent, 0.8));
            }
            if top_edge {
                look.top = thin(accent);
            }
            if header || bottom_edge {
                look.bottom = thin(accent);
            }
        }
        (Family::Light, 1) => {
            if header {
                look.fill = Some(accent);
                look.color = white;
            }
            look.top = top_edge.then(|| thin(accent)).flatten();
            look.bottom = (bottom_edge || banded).then(|| thin(accent)).flatten();
            look.left = (addr.col() == table.range.start().col())
                .then(|| thin(accent))
                .flatten();
            look.right = (addr.col() == table.range.end().col())
                .then(|| thin(accent))
                .flatten();
        }
        (Family::Light, _) => {
            if banded {
                look.fill = Some(tint(accent, 0.8));
            }
            look.top = thin(accent);
            look.bottom = thin(accent);
            look.left = thin(accent);
            look.right = thin(accent);
        }
        (Family::Medium, 0) => {
            if header {
                look.fill = Some(accent);
                look.color = white;
            } else {
                if banded {
                    look.fill = Some(tint(accent, 0.8));
                }
                look.top = thin(tint(accent, 0.4));
                look.bottom = thin(tint(accent, 0.4));
            }
        }
        (Family::Medium, 1) => {
            look.fill = Some(if header {
                accent
            } else if banded {
                tint(accent, 0.6)
            } else {
                tint(accent, 0.8)
            });
            if header {
                look.color = white;
            }
            look.bottom = thin(Color::WHITE);
            look.right = thin(Color::WHITE);
        }
        (Family::Medium, 2) => {
            if header {
                look.fill = Some(Color::BLACK);
                look.color = white;
            } else if banded {
                look.fill = Some(tint(accent, 0.8));
            }
            look.top = top_edge
                .then(|| line(LineStyle::Thick, Color::BLACK))
                .flatten();
            look.bottom = bottom_edge
                .then(|| line(LineStyle::Thick, Color::BLACK))
                .flatten();
        }
        (Family::Medium, _) => {
            look.fill = Some(if banded {
                tint(accent, 0.6)
            } else {
                tint(accent, 0.8)
            });
            look.top = thin(tint(accent, 0.4));
            look.bottom = thin(tint(accent, 0.4));
            look.left = thin(tint(accent, 0.4));
            look.right = thin(tint(accent, 0.4));
        }
        (Family::Dark, 0) => {
            look.color = white;
            look.fill = Some(if header {
                Color::BLACK
            } else if banded {
                tint(accent, -0.25)
            } else if accent == Color::BLACK {
                Color(0x595959)
            } else {
                accent
            });
        }
        (Family::Dark, _) => {
            if header {
                look.fill = Some(Color::BLACK);
                look.color = white;
            } else {
                look.fill = Some(if banded {
                    tint(accent, 0.6)
                } else {
                    tint(accent, 0.8)
                });
            }
        }
    }
    if totals {
        look.top = line(LineStyle::Double, accent);
        if id.family == Family::Dark {
            look.fill = Some(Color::BLACK);
            look.color = white;
        }
    }
    Some(look)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::TableColumn;

    #[test]
    fn style_names_round_trip() {
        assert_eq!(StyleId::all().count(), 60);
        for id in StyleId::all() {
            assert_eq!(StyleId::parse(&id.name()), Some(id));
        }
        assert_eq!(StyleId::parse("TableStyleMedium29"), None);
    }

    #[test]
    fn medium_two_matches_the_default_look() {
        let mut table = Table::new(
            "Tablo1",
            "A1:B4".parse().unwrap(),
            vec![TableColumn::new("A"), TableColumn::new("B")],
        );
        let look = |s: &str, table: &Table| table_look(table, s.parse().unwrap()).unwrap();
        assert_eq!(look("A1", &table).fill, Some(Color(0x4472C4)));
        assert_eq!(look("A1", &table).color, Some(Color::WHITE));
        assert!(look("A1", &table).bold);
        assert_eq!(look("A2", &table).fill, Some(Color(0xDAE3F3)));
        assert_eq!(look("A3", &table).fill, None);
        table.banded_rows = false;
        assert_eq!(look("A2", &table).fill, None);
        assert!(table_look(&table, "C1".parse().unwrap()).is_none());
    }
}
