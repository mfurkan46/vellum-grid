use crate::numfmt::{self, Kind};
use crate::{Cell, CellAddr, CellRange, CellValue, Locale, Sheet, fold};

enum Series {
    Linear {
        start: f64,
        step: f64,
    },
    Numbered {
        prefix: String,
        start: f64,
        step: f64,
        width: usize,
    },
    List {
        names: &'static [&'static str],
        start: i64,
        step: i64,
    },
    Cycle,
}

pub fn fill_series(
    sheet: &Sheet,
    source: CellRange,
    target: CellRange,
    locale: &Locale,
    copy: bool,
) -> Vec<(CellAddr, Cell)> {
    let vertical = target.cols() == source.cols();
    let (lines, length) = if vertical {
        (source.start().col()..=source.end().col(), source.rows())
    } else {
        (source.start().row()..=source.end().row(), source.cols())
    };
    let (first, last) = if vertical {
        (target.start().row(), target.end().row())
    } else {
        (target.start().col(), target.end().col())
    };
    let origin = if vertical {
        source.start().row()
    } else {
        source.start().col()
    };
    let mut edits = Vec::new();
    for line in lines {
        let at = |position: u32| {
            let (row, col) = if vertical {
                (position, line)
            } else {
                (line, position)
            };
            CellAddr::new(row, col).expect("inside the target range")
        };
        let cells: Vec<Cell> = (0..length)
            .map(|i| sheet.get(at(origin + i)).cloned().unwrap_or_default())
            .collect();
        let series = if copy {
            Series::Cycle
        } else {
            detect(&cells, locale)
        };
        for position in first..=last {
            let index = i64::from(position) - i64::from(origin);
            if (0..i64::from(length)).contains(&index) {
                continue;
            }
            let cell = generate(&series, &cells, index, vertical);
            edits.push((at(position), cell));
        }
    }
    edits
}

fn detect(cells: &[Cell], locale: &Locale) -> Series {
    if cells.iter().any(|cell| cell.formula.is_some()) {
        return Series::Cycle;
    }
    let numbers: Option<Vec<f64>> = cells
        .iter()
        .map(|cell| match cell.value {
            CellValue::Number(n) => Some(n),
            _ => None,
        })
        .collect();
    if let Some(numbers) = numbers {
        return match numbers.as_slice() {
            [single] if numfmt::kind(cells[0].number_format()) == Kind::Date => Series::Linear {
                start: *single,
                step: 1.0,
            },
            [_] => Series::Cycle,
            values => {
                let (start, step) = trend(values);
                Series::Linear { start, step }
            }
        };
    }
    let texts: Option<Vec<&str>> = cells
        .iter()
        .map(|cell| match &cell.value {
            CellValue::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    let Some(texts) = texts else {
        return Series::Cycle;
    };
    if let Some(series) = list_series(&texts, locale) {
        return series;
    }
    let parts: Option<Vec<(&str, &str)>> = texts.iter().map(|text| split_number(text)).collect();
    if let Some(parts) = parts
        && parts.iter().all(|(prefix, _)| *prefix == parts[0].0)
    {
        let values: Vec<f64> = parts
            .iter()
            .map(|(_, digits)| digits.parse().unwrap_or(0.0))
            .collect();
        let (start, step) = if values.len() == 1 {
            (values[0], 1.0)
        } else {
            trend(&values)
        };
        return Series::Numbered {
            prefix: parts[0].0.to_owned(),
            start,
            step,
            width: parts[0].1.len(),
        };
    }
    Series::Cycle
}

fn list_series(texts: &[&str], locale: &Locale) -> Option<Series> {
    let lists: [&'static [&'static str]; 4] =
        [&TR_MONTHS, &TR_MONTHS_SHORT, &TR_DAYS, &TR_DAYS_SHORT];
    let english: [&'static [&'static str]; 4] =
        [&EN_MONTHS, &EN_MONTHS_SHORT, &EN_DAYS, &EN_DAYS_SHORT];
    let lists = if locale.months[0] == TR_MONTHS[0] {
        lists
    } else {
        english
    };
    lists.into_iter().find_map(|names| {
        let indices: Option<Vec<i64>> = texts
            .iter()
            .map(|text| {
                names
                    .iter()
                    .position(|name| fold(name) == fold(text))
                    .map(|i| i as i64)
            })
            .collect();
        let indices = indices?;
        let step = match indices.as_slice() {
            [_] => 1,
            [a, b, ..] => b - a,
            [] => return None,
        };
        Some(Series::List {
            names,
            start: indices[0],
            step,
        })
    })
}

fn split_number(text: &str) -> Option<(&str, &str)> {
    let start = text
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_digit())
        .last()?
        .0;
    Some((&text[..start], &text[start..]))
}

fn trend(values: &[f64]) -> (f64, f64) {
    let n = values.len() as f64;
    let mean_x = (n - 1.0) / 2.0;
    let mean_y = values.iter().sum::<f64>() / n;
    let (mut covariance, mut variance) = (0.0, 0.0);
    for (i, y) in values.iter().enumerate() {
        let dx = i as f64 - mean_x;
        covariance += dx * (y - mean_y);
        variance += dx * dx;
    }
    let step = if variance == 0.0 {
        0.0
    } else {
        covariance / variance
    };
    (mean_y - step * mean_x, step)
}

fn generate(series: &Series, cells: &[Cell], index: i64, vertical: bool) -> Cell {
    let length = cells.len() as i64;
    let slot = index.rem_euclid(length);
    let template = &cells[slot as usize];
    let with_value = |value: CellValue| Cell {
        value,
        formula: None,
        style: template.style.clone(),
    };
    let at = index as f64;
    match series {
        Series::Linear { start, step } => with_value(CellValue::Number(
            crate::value::round_significant(start + step * at),
        )),
        Series::Numbered {
            prefix,
            start,
            step,
            width,
        } => {
            let number = (start + step * at).round();
            let digits = format!("{:0width$}", number.abs() as u64, width = *width);
            let text = if number < 0.0 {
                format!("{prefix}-{digits}")
            } else {
                format!("{prefix}{digits}")
            };
            with_value(CellValue::Text(text))
        }
        Series::List { names, start, step } => {
            let position = (start + step * index).rem_euclid(names.len() as i64);
            with_value(CellValue::Text(names[position as usize].to_owned()))
        }
        Series::Cycle => {
            let offset = index - slot;
            if vertical {
                template.shifted(offset, 0)
            } else {
                template.shifted(0, offset)
            }
        }
    }
}

const TR_MONTHS: [&str; 12] = [
    "Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim",
    "Kasım", "Aralık",
];
const TR_MONTHS_SHORT: [&str; 12] = [
    "Oca", "Şub", "Mar", "Nis", "May", "Haz", "Tem", "Ağu", "Eyl", "Eki", "Kas", "Ara",
];
const TR_DAYS: [&str; 7] = [
    "Pazartesi",
    "Salı",
    "Çarşamba",
    "Perşembe",
    "Cuma",
    "Cumartesi",
    "Pazar",
];
const TR_DAYS_SHORT: [&str; 7] = ["Pzt", "Sal", "Çar", "Per", "Cum", "Cmt", "Paz"];
const EN_MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const EN_MONTHS_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const EN_DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const EN_DAYS_SHORT: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TR;

    fn sheet(inputs: &[(&str, &str)]) -> Sheet {
        let mut sheet = Sheet::new("s");
        for (at, input) in inputs {
            sheet.set(at.parse().unwrap(), Cell::parse_in(input, &TR).unwrap());
        }
        sheet
    }

    fn filled(inputs: &[(&str, &str)], source: &str, target: &str, copy: bool) -> Vec<String> {
        let sheet = sheet(inputs);
        fill_series(
            &sheet,
            source.parse().unwrap(),
            target.parse().unwrap(),
            &TR,
            copy,
        )
        .into_iter()
        .map(|(_, cell)| cell.input_text_in(&TR))
        .collect()
    }

    #[test]
    fn numbers_follow_the_trend() {
        assert_eq!(
            filled(&[("A1", "1"), ("A2", "3")], "A1:A2", "A1:A5", false),
            ["5", "7", "9"]
        );
        assert_eq!(
            filled(&[("A1", "0,1"), ("A2", "0,2")], "A1:A2", "A1:A4", false),
            ["0,3", "0,4"]
        );
        assert_eq!(filled(&[("A1", "7")], "A1", "A1:A3", false), ["7", "7"]);
        assert_eq!(
            filled(&[("A1", "1"), ("A2", "2")], "A1:A2", "A1:A4", true),
            ["1", "2"]
        );
    }

    #[test]
    fn fills_backwards_and_sideways() {
        assert_eq!(
            filled(&[("C1", "10"), ("D1", "20")], "C1:D1", "A1:D1", false),
            ["-10", "0"]
        );
        assert_eq!(
            filled(&[("A1", "1"), ("B1", "2")], "A1:B1", "A1:D1", false),
            ["3", "4"]
        );
    }

    #[test]
    fn dates_and_lists_increment() {
        assert_eq!(
            filled(&[("A1", "30.12.2026")], "A1", "A1:A3", false),
            ["31.12.2026", "01.01.2027"]
        );
        assert_eq!(
            filled(&[("A1", "Kasım")], "A1", "A1:A4", false),
            ["Aralık", "Ocak", "Şubat"]
        );
        assert_eq!(
            filled(&[("A1", "pzt"), ("A2", "Çar")], "A1:A2", "A1:A4", false),
            ["Cum", "Paz"]
        );
    }

    #[test]
    fn numbered_text_increments() {
        assert_eq!(
            filled(&[("A1", "Ürün 1")], "A1", "A1:A3", false),
            ["Ürün 2", "Ürün 3"]
        );
        assert_eq!(filled(&[("A1", "K007")], "A1", "A1:A2", false), ["K008"]);
        assert_eq!(
            filled(&[("A1", "elma")], "A1", "A1:A3", false),
            ["elma", "elma"]
        );
    }

    #[test]
    fn formulas_shift_while_copying() {
        assert_eq!(
            filled(&[("B1", "=A1*2")], "B1", "B1:B3", false),
            ["=A2*2", "=A3*2"]
        );
        assert_eq!(
            filled(&[("A1", "=$C$1+B1")], "A1", "A1:C1", false),
            ["=$C$1+C1", "=$C$1+D1"]
        );
    }

    #[test]
    fn mixed_sources_repeat() {
        assert_eq!(
            filled(&[("A1", "a"), ("A2", "1")], "A1:A2", "A1:A5", false),
            ["a", "1", "a"]
        );
    }
}
