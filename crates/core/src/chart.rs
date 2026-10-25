use crate::numfmt::{self, Kind};
use crate::{CellAddr, CellRange, CellValue, Color, Locale, Sheet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartKind {
    Column,
    Bar,
    Line,
    Pie,
    Scatter,
    Area,
    Doughnut,
}

impl ChartKind {
    pub fn is_round(self) -> bool {
        matches!(self, Self::Pie | Self::Doughnut)
    }

    pub fn stacks(self) -> bool {
        matches!(self, Self::Column | Self::Bar | Self::Line | Self::Area)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Grouping {
    #[default]
    Clustered,
    Stacked,
    Percent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Legend {
    #[default]
    Auto,
    Hidden,
    Bottom,
    Top,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chart {
    pub kind: ChartKind,
    pub data: CellRange,
    pub title: Option<String>,
    pub anchor: CellAddr,
    pub offset: (u32, u32),
    pub size: (u32, u32),
    pub grouping: Grouping,
    pub legend: Legend,
    pub labels: bool,
    pub x_title: Option<String>,
    pub y_title: Option<String>,
    pub gridlines: bool,
    pub markers: bool,
}

impl Chart {
    pub fn new(kind: ChartKind, data: CellRange, anchor: CellAddr) -> Self {
        Self {
            kind,
            data,
            title: None,
            anchor,
            offset: (0, 0),
            size: DEFAULT_CHART_SIZE,
            grouping: Grouping::Clustered,
            legend: Legend::Auto,
            labels: false,
            x_title: None,
            y_title: None,
            gridlines: true,
            markers: false,
        }
    }

    pub fn legend_shown(&self, data: &ChartData) -> bool {
        match self.legend {
            Legend::Auto => self.kind.is_round() || data.series.len() > 1,
            Legend::Hidden => false,
            _ => true,
        }
    }
}

pub const DEFAULT_CHART_SIZE: (u32, u32) = (480, 288);

pub fn stacked(data: &ChartData, grouping: Grouping) -> Vec<Vec<Option<(f64, f64)>>> {
    let count = data
        .series
        .iter()
        .map(|s| s.values.len())
        .max()
        .unwrap_or(0);
    let mut positive = vec![0.0; count];
    let mut negative = vec![0.0; count];
    let totals: Vec<f64> = (0..count)
        .map(|i| {
            data.series
                .iter()
                .filter_map(|s| s.values.get(i).copied().flatten())
                .map(f64::abs)
                .sum()
        })
        .collect();
    data.series
        .iter()
        .map(|series| {
            (0..count)
                .map(|i| {
                    let value = series.values.get(i).copied().flatten()?;
                    if grouping == Grouping::Clustered {
                        return Some((0.0, value));
                    }
                    let value = if grouping == Grouping::Percent {
                        if totals[i] == 0.0 {
                            0.0
                        } else {
                            value / totals[i]
                        }
                    } else {
                        value
                    };
                    let base = if value < 0.0 {
                        &mut negative[i]
                    } else {
                        &mut positive[i]
                    };
                    let start = *base;
                    *base += value;
                    Some((start, *base))
                })
                .collect()
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SparkKind {
    Line,
    Column,
    WinLoss,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SparklineGroup {
    pub kind: SparkKind,
    pub cells: Vec<(CellAddr, CellRange)>,
    pub color: Color,
    pub point_color: Color,
    pub markers: bool,
    pub high: bool,
    pub low: bool,
    pub first: bool,
    pub last: bool,
    pub negative: bool,
}

impl SparklineGroup {
    pub fn new(kind: SparkKind, cells: Vec<(CellAddr, CellRange)>) -> Self {
        Self {
            kind,
            cells,
            color: Color(0x376092),
            point_color: Color(0xD00000),
            markers: false,
            high: false,
            low: false,
            first: false,
            last: false,
            negative: kind == SparkKind::WinLoss,
        }
    }

    pub fn at(&self, addr: CellAddr) -> Option<CellRange> {
        self.cells
            .iter()
            .find(|(location, _)| *location == addr)
            .map(|(_, data)| *data)
    }
}

pub fn spark_values(sheet: &Sheet, data: CellRange) -> Vec<Option<f64>> {
    let mut out = Vec::new();
    for row in data.start().row()..=data.end().row() {
        for col in data.start().col()..=data.end().col() {
            let addr = CellAddr::new(row, col).expect("inside the range");
            out.push(match sheet.value(addr) {
                CellValue::Number(n) => Some(*n),
                _ => None,
            });
        }
    }
    out
}

pub fn spark_pairs(data: CellRange, location: CellRange) -> Option<Vec<(CellAddr, CellRange)>> {
    let count = if location.rows() == 1 {
        location.cols()
    } else if location.cols() == 1 {
        location.rows()
    } else {
        return None;
    };
    let by_rows = if data.rows() == count
        && data.cols() >= 1
        && (data.cols() != count || location.cols() == 1)
    {
        true
    } else if data.cols() == count {
        false
    } else if count == 1 {
        data.rows() == 1 || data.cols() == 1
    } else {
        return None;
    };
    Some(
        (0..count)
            .map(|i| {
                let cell = if location.rows() == 1 {
                    CellAddr::new(location.start().row(), location.start().col() + i)
                } else {
                    CellAddr::new(location.start().row() + i, location.start().col())
                }
                .expect("inside the location");
                let range = if count == 1 {
                    data
                } else if by_rows {
                    let row = data.start().row() + i;
                    CellRange::new(
                        CellAddr::new(row, data.start().col()).expect("inside"),
                        CellAddr::new(row, data.end().col()).expect("inside"),
                    )
                } else {
                    let col = data.start().col() + i;
                    CellRange::new(
                        CellAddr::new(data.start().row(), col).expect("inside"),
                        CellAddr::new(data.end().row(), col).expect("inside"),
                    )
                };
                (cell, range)
            })
            .collect(),
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub name: String,
    pub name_cell: Option<CellAddr>,
    pub values: Vec<Option<f64>>,
    pub range: CellRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChartData {
    pub categories: Vec<String>,
    pub numbers: Vec<Option<f64>>,
    pub categories_range: Option<CellRange>,
    pub series: Vec<Series>,
}

pub fn chart_data(sheet: &Sheet, data: CellRange, locale: &Locale) -> ChartData {
    let by_columns = data.rows() >= data.cols();
    let (lines, length) = if by_columns {
        (data.cols(), data.rows())
    } else {
        (data.rows(), data.cols())
    };
    let at = |line: u32, index: u32| {
        let (row, col) = if by_columns {
            (data.start().row() + index, data.start().col() + line)
        } else {
            (data.start().row() + line, data.start().col() + index)
        };
        CellAddr::new(row, col).expect("inside the data range")
    };
    let label = |addr: CellAddr| match sheet.get(addr) {
        None => None,
        Some(cell) => match &cell.value {
            CellValue::Text(_) => Some(true),
            CellValue::Number(_) if numfmt::kind(cell.number_format()) == Kind::Date => Some(true),
            CellValue::Empty => None,
            _ => Some(false),
        },
    };
    let all_labels = |cells: Vec<CellAddr>| {
        let kinds: Vec<bool> = cells.into_iter().filter_map(label).collect();
        !kinds.is_empty() && kinds.into_iter().all(|is_label| is_label)
    };
    let corner_empty = label(at(0, 0)).is_none();
    let names = length > 1 && lines > 0 && {
        let headers: Vec<CellAddr> = (0..lines).map(|line| at(line, 0)).collect();
        let rest: Vec<CellAddr> = headers.iter().copied().skip(1).collect();
        all_labels(if corner_empty { rest } else { headers })
    };
    let first = u32::from(names);
    let categories_line = lines > 1
        && (corner_empty && names || all_labels((first..length).map(|i| at(0, i)).collect()));
    let first_series = u32::from(categories_line);
    let text = |addr: CellAddr| {
        sheet
            .get(addr)
            .map(|cell| locale.format_value(&cell.value, cell.number_format()))
            .unwrap_or_default()
    };
    let number = |addr: CellAddr| match sheet.value(addr) {
        CellValue::Number(n) => Some(*n),
        _ => None,
    };
    let (categories, numbers) = if categories_line {
        (first..length)
            .map(|i| (text(at(0, i)), number(at(0, i))))
            .unzip()
    } else {
        (first..length)
            .map(|i| ((i - first + 1).to_string(), Some(f64::from(i - first + 1))))
            .unzip()
    };
    let series = (first_series..lines)
        .map(|line| {
            let name_cell = names.then(|| at(line, 0));
            let name = name_cell
                .map(text)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| format!("{}{}", locale.series_name, line - first_series + 1));
            Series {
                name,
                name_cell,
                values: (first..length).map(|i| number(at(line, i))).collect(),
                range: CellRange::new(at(line, first.min(length - 1)), at(line, length - 1)),
            }
        })
        .collect();
    ChartData {
        categories,
        numbers,
        categories_range: categories_line
            .then(|| CellRange::new(at(0, first.min(length - 1)), at(0, length - 1))),
        series,
    }
}

pub fn nice_scale(min: f64, max: f64, ticks: usize) -> (f64, f64, f64) {
    let (low, high) = if min == max {
        if min == 0.0 {
            (0.0, 1.0)
        } else {
            (min.min(0.0), max.max(0.0))
        }
    } else {
        (min, max)
    };
    let raw = (high - low) / ticks.max(1) as f64;
    let magnitude = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .into_iter()
        .map(|factor| factor * magnitude)
        .find(|step| *step >= raw)
        .unwrap_or(10.0 * magnitude);
    let start = (low / step).floor() * step;
    let end = (high / step).ceil() * step;
    (start, end.max(start + step), step)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cell, TR};

    fn sheet(inputs: &[(&str, &str)]) -> Sheet {
        let mut sheet = Sheet::new("s");
        for (at, input) in inputs {
            sheet.set(at.parse().unwrap(), Cell::parse_in(input, &TR).unwrap());
        }
        sheet
    }

    #[test]
    fn headers_and_categories_are_detected() {
        let s = sheet(&[
            ("B1", "Gelir"),
            ("C1", "Gider"),
            ("A2", "Ocak"),
            ("B2", "10"),
            ("C2", "4"),
            ("A3", "Şubat"),
            ("B3", "12"),
            ("C3", "6"),
        ]);
        let data = chart_data(&s, "A1:C3".parse().unwrap(), &TR);
        assert_eq!(data.categories, ["Ocak", "Şubat"]);
        assert_eq!(data.series.len(), 2);
        assert_eq!(data.series[0].name, "Gelir");
        assert_eq!(data.series[1].values, [Some(4.0), Some(6.0)]);
        assert_eq!(data.series[0].range, "B2:B3".parse().unwrap());
        assert_eq!(data.categories_range, Some("A2:A3".parse().unwrap()));
    }

    #[test]
    fn plain_numbers_make_numbered_series() {
        let s = sheet(&[("A1", "3"), ("A2", "5"), ("A3", "7")]);
        let data = chart_data(&s, "A1:A3".parse().unwrap(), &TR);
        assert_eq!(data.categories, ["1", "2", "3"]);
        assert_eq!(data.series[0].name, "Seri1");
        assert_eq!(data.series[0].values, [Some(3.0), Some(5.0), Some(7.0)]);
    }

    #[test]
    fn wide_ranges_use_rows_as_series() {
        let s = sheet(&[
            ("A1", "Ad"),
            ("B1", "Q1"),
            ("C1", "Q2"),
            ("D1", "Q3"),
            ("A2", "Satış"),
            ("B2", "1"),
            ("C2", "2"),
            ("D2", "3"),
        ]);
        let data = chart_data(&s, "A1:D2".parse().unwrap(), &TR);
        assert_eq!(data.categories, ["Q1", "Q2", "Q3"]);
        assert_eq!(data.series.len(), 1);
        assert_eq!(data.series[0].name, "Satış");
        assert_eq!(data.series[0].values, [Some(1.0), Some(2.0), Some(3.0)]);
    }

    #[test]
    fn stacking_adds_up_and_normalizes() {
        let s = sheet(&[("A1", "1"), ("B1", "3"), ("A2", "-2"), ("B2", "2")]);
        let data = chart_data(&s, "A1:B2".parse().unwrap(), &TR);
        let stacked_values = stacked(&data, Grouping::Stacked);
        assert_eq!(stacked_values[0][0], Some((0.0, 1.0)));
        assert_eq!(stacked_values[1][0], Some((1.0, 4.0)));
        assert_eq!(stacked_values[0][1], Some((0.0, -2.0)));
        assert_eq!(stacked_values[1][1], Some((0.0, 2.0)));
        let percent = stacked(&data, Grouping::Percent);
        assert_eq!(percent[1][0], Some((0.25, 1.0)));
        assert_eq!(stacked(&data, Grouping::Clustered)[1][1], Some((0.0, 2.0)));
    }

    #[test]
    fn sparklines_pair_rows_with_locations() {
        let pairs = spark_pairs("A1:E3".parse().unwrap(), "F1:F3".parse().unwrap()).unwrap();
        assert_eq!(pairs[1], ("F2".parse().unwrap(), "A2:E2".parse().unwrap()));
        let pairs = spark_pairs("A1:C5".parse().unwrap(), "A6:C6".parse().unwrap()).unwrap();
        assert_eq!(pairs[2], ("C6".parse().unwrap(), "C1:C5".parse().unwrap()));
        assert!(spark_pairs("A1:E3".parse().unwrap(), "F1:F2".parse().unwrap()).is_none());
        let single = spark_pairs("A1:E1".parse().unwrap(), "F1".parse().unwrap()).unwrap();
        assert_eq!(
            single,
            vec![("F1".parse().unwrap(), "A1:E1".parse().unwrap())]
        );
        let s = sheet(&[("A1", "2"), ("C1", "5")]);
        assert_eq!(
            spark_values(&s, "A1:C1".parse().unwrap()),
            [Some(2.0), None, Some(5.0)]
        );
    }

    #[test]
    fn nice_scales() {
        assert_eq!(nice_scale(0.0, 93.0, 5), (0.0, 100.0, 20.0));
        assert_eq!(nice_scale(-3.0, 7.0, 5), (-4.0, 8.0, 2.0));
        assert_eq!(nice_scale(5.0, 5.0, 5), (0.0, 5.0, 1.0));
        assert_eq!(nice_scale(0.0, 0.0, 5), (0.0, 1.0, 0.2));
    }
}
