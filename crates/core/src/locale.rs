use crate::date::{date_serial, from_serial, time_fraction};
use crate::numfmt::{self, Kind, Rendered};
use crate::value::{parse_canonical_number, round_significant};
use crate::{CellError, CellValue, format_number, format_number_fit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateStyle {
    Iso,
    DayMonthYear,
    MonthDayYear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Collation {
    Binary,
    Turkish,
}

#[derive(Debug)]
pub struct Locale {
    pub date: DateStyle,
    pub collation: Collation,
    pub decimal: char,
    pub group: char,
    pub list: char,
    pub true_name: &'static str,
    pub false_name: &'static str,
    pub months: [&'static str; 12],
    pub months_short: [&'static str; 12],
    pub days: [&'static str; 7],
    pub days_short: [&'static str; 7],
    pub am: &'static str,
    pub pm: &'static str,
    pub series_name: &'static str,
    pub table_items: [&'static str; 5],
    pub table_words: [&'static str; 3],
    errors: [&'static str; 7],
    functions: &'static [(&'static str, &'static str)],
}

pub const EN: Locale = Locale {
    date: DateStyle::Iso,
    collation: Collation::Binary,
    decimal: '.',
    group: ',',
    list: ',',
    true_name: "TRUE",
    false_name: "FALSE",
    months: [
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
    ],
    months_short: [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ],
    days: [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ],
    days_short: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
    am: "AM",
    pm: "PM",
    series_name: "Series",
    table_words: ["Table", "Column", "Total"],
    table_items: ["#All", "#Data", "#Headers", "#Totals", "#This Row"],
    errors: [
        "#NULL!", "#DIV/0!", "#VALUE!", "#REF!", "#NAME?", "#NUM!", "#N/A",
    ],
    functions: &[],
};

pub const EN_US: Locale = Locale {
    date: DateStyle::MonthDayYear,
    ..EN
};

pub const TR: Locale = Locale {
    date: DateStyle::DayMonthYear,
    collation: Collation::Turkish,
    decimal: ',',
    group: '.',
    list: ';',
    true_name: "DOĞRU",
    false_name: "YANLIŞ",
    months: [
        "Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim",
        "Kasım", "Aralık",
    ],
    months_short: [
        "Oca", "Şub", "Mar", "Nis", "May", "Haz", "Tem", "Ağu", "Eyl", "Eki", "Kas", "Ara",
    ],
    days: [
        "Pazar",
        "Pazartesi",
        "Salı",
        "Çarşamba",
        "Perşembe",
        "Cuma",
        "Cumartesi",
    ],
    days_short: ["Paz", "Pzt", "Sal", "Çar", "Per", "Cum", "Cmt"],
    am: "ÖÖ",
    pm: "ÖS",
    series_name: "Seri",
    table_words: ["Tablo", "Sütun", "Toplam"],
    table_items: ["#Tümü", "#Veri", "#Başlıklar", "#Toplamlar", "#Bu Satır"],
    errors: [
        "#BOŞ!", "#SAYI/0!", "#DEĞER!", "#BAŞV!", "#AD?", "#SAYI!", "#YOK",
    ],
    functions: &[
        ("ABS", "MUTLAK"),
        ("AND", "VE"),
        ("AVERAGE", "ORTALAMA"),
        ("CONCAT", "ARALIKBİRLEŞTİR"),
        ("CONCATENATE", "BİRLEŞTİR"),
        ("COUNT", "BAĞ_DEĞ_SAY"),
        ("COUNTA", "BAĞ_DEĞ_DOLU_SAY"),
        ("COUNTIF", "EĞERSAY"),
        ("IF", "EĞER"),
        ("IFERROR", "EĞERHATA"),
        ("LEFT", "SOLDAN"),
        ("LEN", "UZUNLUK"),
        ("MAX", "MAK"),
        ("MID", "PARÇAAL"),
        ("MIN", "MİN"),
        ("NOT", "DEĞİL"),
        ("OR", "YADA"),
        ("RIGHT", "SAĞDAN"),
        ("ROUND", "YUVARLA"),
        ("SUM", "TOPLA"),
        ("SUMIF", "ETOPLA"),
        ("TODAY", "BUGÜN"),
        ("VLOOKUP", "DÜŞEYARA"),
        ("XLOOKUP", "ÇAPRAZARA"),
        ("AVERAGEIF", "EĞERORTALAMA"),
        ("AVERAGEIFS", "ÇOKEĞERORTALAMA"),
        ("CEILING", "TAVANAYUVARLA"),
        ("CHAR", "DAMGA"),
        ("CHOOSE", "ELEMAN"),
        ("CLEAN", "TEMİZ"),
        ("CODE", "KOD"),
        ("COLUMN", "SÜTUN"),
        ("COLUMNS", "SÜTUNSAY"),
        ("COUNTBLANK", "BOŞLUKSAY"),
        ("COUNTIFS", "ÇOKEĞERSAY"),
        ("DATE", "TARİH"),
        ("DATEDIF", "ETARİHLİ"),
        ("DATEVALUE", "TARİHSAYISI"),
        ("DAY", "GÜN"),
        ("DAYS", "GÜNSAY"),
        ("EDATE", "SERİTARİH"),
        ("EOMONTH", "SERİAY"),
        ("EXACT", "ÖZDEŞ"),
        ("EXP", "ÜS"),
        ("FALSE", "YANLIŞ"),
        ("FIND", "BUL"),
        ("FLOOR", "TABANAYUVARLA"),
        ("HLOOKUP", "YATAYARA"),
        ("HOUR", "SAAT"),
        ("IFNA", "EĞERYOKSA"),
        ("IFS", "ÇOKEĞER"),
        ("INDEX", "İNDİS"),
        ("INDIRECT", "DOLAYLI"),
        ("INT", "TAMSAYI"),
        ("ISBLANK", "EBOŞSA"),
        ("ISERR", "EHATA"),
        ("ISERROR", "EHATALIYSA"),
        ("ISLOGICAL", "EMANTIKSALSA"),
        ("ISNA", "EYOKSA"),
        ("ISNUMBER", "ESAYIYSA"),
        ("ISTEXT", "EMETİNSE"),
        ("LARGE", "BÜYÜK"),
        ("LOWER", "KÜÇÜKHARF"),
        ("MATCH", "KAÇINCI"),
        ("MAXIFS", "ÇOKEĞERMAK"),
        ("MEDIAN", "ORTANCA"),
        ("MINIFS", "ÇOKEĞERMİN"),
        ("MINUTE", "DAKİKA"),
        ("MONTH", "AY"),
        ("MROUND", "KYUVARLA"),
        ("NA", "YOKSAY"),
        ("NETWORKDAYS", "TAMİŞGÜNÜ"),
        ("NOW", "ŞİMDİ"),
        ("OFFSET", "KAYDIR"),
        ("PI", "Pİ"),
        ("POWER", "KUVVET"),
        ("PRODUCT", "ÇARPIM"),
        ("PROPER", "YAZIM.DÜZENİ"),
        ("RAND", "S_SAYI_ÜRET"),
        ("RANDBETWEEN", "RASTGELEARADA"),
        ("REPLACE", "DEĞİŞTİR"),
        ("REPT", "YİNELE"),
        ("ROUNDDOWN", "AŞAĞIYUVARLA"),
        ("ROUNDUP", "YUKARIYUVARLA"),
        ("ROW", "SATIR"),
        ("ROWS", "SATIRSAY"),
        ("SEARCH", "MBUL"),
        ("SECOND", "SANİYE"),
        ("SIGN", "İŞARET"),
        ("SMALL", "KÜÇÜK"),
        ("SQRT", "KAREKÖK"),
        ("STDEV", "STDSAPMA"),
        ("SUBSTITUTE", "YERİNEKOY"),
        ("SUBTOTAL", "ALTTOPLAM"),
        ("SUMIFS", "ÇOKETOPLA"),
        ("SUMPRODUCT", "TOPLA.ÇARPIM"),
        ("SWITCH", "İLKEŞLEŞEN"),
        ("TEXT", "METNEÇEVİR"),
        ("TEXTJOIN", "METİNBİRLEŞTİR"),
        ("TIME", "ZAMAN"),
        ("TRIM", "KIRP"),
        ("TRUE", "DOĞRU"),
        ("TRUNC", "NSAT"),
        ("UPPER", "BÜYÜKHARF"),
        ("VALUE", "SAYIYAÇEVİR"),
        ("WEEKDAY", "HAFTANINGÜNÜ"),
        ("WEEKNUM", "HAFTASAY"),
        ("WORKDAY", "İŞGÜNÜ"),
        ("XOR", "ÖZELVEYA"),
        ("YEAR", "YIL"),
    ],
};

const ERRORS: [CellError; 7] = [
    CellError::Null,
    CellError::Div0,
    CellError::Value,
    CellError::Ref,
    CellError::Name,
    CellError::Num,
    CellError::NA,
];

impl Locale {
    pub fn collate(&self, a: &str, b: &str) -> std::cmp::Ordering {
        let turkish = self.collation == Collation::Turkish;
        let key = |text: &str| -> Vec<(char, u8)> {
            text.chars()
                .flat_map(|c| match c {
                    'I' if turkish => vec!['ı'],
                    'İ' if turkish => vec!['i'],
                    c => c.to_lowercase().collect(),
                })
                .map(|c| match c {
                    'ç' if turkish => ('c', 1),
                    'ğ' if turkish => ('g', 1),
                    'ı' if turkish => ('i', 0),
                    'i' if turkish => ('i', 1),
                    'ö' if turkish => ('o', 1),
                    'ş' if turkish => ('s', 1),
                    'ü' if turkish => ('u', 1),
                    c => (c, 0),
                })
                .collect()
        };
        key(a).cmp(&key(b))
    }

    pub fn error_name(&self, error: CellError) -> &'static str {
        let index = ERRORS
            .iter()
            .position(|e| *e == error)
            .expect("all errors listed");
        self.errors[index]
    }

    pub fn parse_error(&self, s: &str) -> Option<CellError> {
        let key = fold(s);
        ERRORS
            .into_iter()
            .find(|e| fold(self.error_name(*e)) == key)
    }

    pub fn errors(&self) -> impl Iterator<Item = (CellError, &'static str)> + '_ {
        ERRORS.into_iter().map(|e| (e, self.error_name(e)))
    }

    pub fn bool_name(&self, value: bool) -> &'static str {
        if value {
            self.true_name
        } else {
            self.false_name
        }
    }

    pub fn parse_bool(&self, s: &str) -> Option<bool> {
        let key = fold(s);
        if key == fold(self.true_name) {
            Some(true)
        } else if key == fold(self.false_name) {
            Some(false)
        } else {
            None
        }
    }

    pub fn upper(&self, text: &str) -> String {
        if self.collation != Collation::Turkish {
            return text.to_uppercase();
        }
        text.chars()
            .flat_map(|c| match c {
                'i' => vec!['İ'],
                'ı' => vec!['I'],
                c => c.to_uppercase().collect(),
            })
            .collect()
    }

    pub fn lower(&self, text: &str) -> String {
        if self.collation != Collation::Turkish {
            return text.to_lowercase();
        }
        text.chars()
            .flat_map(|c| match c {
                'I' => vec!['ı'],
                'İ' => vec!['i'],
                c => c.to_lowercase().collect(),
            })
            .collect()
    }

    pub fn format_code(&self, code: &str) -> String {
        if self.collation != Collation::Turkish {
            return code.to_owned();
        }
        let mut quoted = false;
        let dated = code.chars().any(|c| {
            if c == '"' {
                quoted = !quoted;
            }
            !quoted && matches!(c.to_ascii_lowercase(), 'g' | 'a' | 'y' | 's' | 'n')
        });
        let mut quoted = false;
        code.chars()
            .map(|c| {
                if c == '"' {
                    quoted = !quoted;
                }
                if quoted {
                    return c;
                }
                match (dated, c.to_ascii_lowercase()) {
                    (true, 'g') => 'd',
                    (true, 'a') => 'm',
                    (true, 's') => 'h',
                    (true, 'd') => 'm',
                    (true, 'n') => 's',
                    (false, '.') => ',',
                    (false, ',') => '.',
                    _ => c,
                }
            })
            .collect()
    }

    pub fn function_name<'a>(&self, canonical: &'a str) -> &'a str {
        self.functions
            .iter()
            .find(|(name, _)| *name == canonical)
            .map_or(canonical, |(_, local)| local)
    }

    pub fn canonical_function(&self, name: &str) -> String {
        let key = fold(name);
        self.functions
            .iter()
            .find(|(_, local)| fold(local) == key)
            .map_or_else(
                || name.to_uppercase(),
                |(canonical, _)| (*canonical).to_owned(),
            )
    }

    pub fn parse_number(&self, s: &str) -> Option<f64> {
        let (body, percent) = match s.strip_suffix('%') {
            Some(body) => (body, true),
            None => (s, false),
        };
        let n = parse_canonical_number(&self.canonical_number(body)?)?;
        Some(if percent {
            round_significant(n / 100.0)
        } else {
            n
        })
    }

    fn canonical_number(&self, s: &str) -> Option<String> {
        let (mantissa, exponent) = match s.find(['e', 'E']) {
            Some(i) => s.split_at(i),
            None => (s, ""),
        };
        let (sign, digits) = match mantissa.strip_prefix(['+', '-']) {
            Some(rest) => mantissa.split_at(mantissa.len() - rest.len()),
            None => ("", mantissa),
        };
        let (int, frac) = match digits.split_once(self.decimal) {
            Some((int, frac)) => (int, Some(frac)),
            None => (digits, None),
        };
        if frac.is_some_and(|f| f.contains([self.decimal, self.group])) {
            return None;
        }
        let int = if int.contains(self.group) {
            let mut groups = int.split(self.group);
            let first = groups.next()?;
            if first.is_empty() || first.len() > 3 {
                return None;
            }
            let mut joined = first.to_owned();
            for group in groups {
                if group.len() != 3 {
                    return None;
                }
                joined.push_str(group);
            }
            joined
        } else {
            int.to_owned()
        };
        let frac = frac.map(|f| format!(".{f}")).unwrap_or_default();
        Some(format!("{sign}{int}{frac}{exponent}"))
    }

    pub fn format_number(&self, n: f64) -> String {
        self.localize_number(format_number(n))
    }

    pub fn format_number_fit(&self, n: f64, max_len: usize) -> Option<String> {
        format_number_fit(n, max_len).map(|s| self.localize_number(s))
    }

    fn localize_number(&self, s: String) -> String {
        if self.decimal == '.' {
            s
        } else {
            s.replace('.', &self.decimal.to_string())
        }
    }

    pub fn parse_date_time(&self, s: &str) -> Option<(f64, &'static str)> {
        let s = s.trim();
        let (date, time) = match s.split_once(' ') {
            Some((date, time)) => (Some(date), Some(time.trim())),
            None if s.contains(':') => (None, Some(s)),
            None => (Some(s), None),
        };
        let date = match date {
            Some(text) => Some(self.parse_date(text)?),
            None => None,
        };
        let time = match time {
            Some(text) => Some(parse_time(text)?),
            None => None,
        };
        match (date, time) {
            (Some(d), None) => Some((d, numfmt::DATE)),
            (None, Some((t, false))) => Some((t, numfmt::TIME)),
            (None, Some((t, true))) => Some((t, numfmt::TIME_SECONDS)),
            (Some(d), Some((t, _))) => Some((d + t, numfmt::DATE_TIME)),
            (None, None) => None,
        }
    }

    fn parse_date(&self, s: &str) -> Option<f64> {
        let parts: Vec<&str> = s.split(['.', '/', '-']).collect();
        let numeric =
            |p: &&str| !p.is_empty() && p.len() <= 4 && p.bytes().all(|b| b.is_ascii_digit());
        if parts.len() != 3 || !parts.iter().all(numeric) {
            return None;
        }
        let n = |i: usize| parts[i].parse::<u32>().ok();
        let (year, month, day) = if parts[0].len() == 4 && s.contains('-') {
            (i64::from(n(0)?), n(1)?, n(2)?)
        } else if self.date != DateStyle::Iso && parts[0].len() <= 2 && parts[1].len() <= 2 {
            let year = match parts[2].len() {
                4 => i64::from(n(2)?),
                2 if n(2)? < 30 => 2000 + i64::from(n(2)?),
                2 => 1900 + i64::from(n(2)?),
                _ => return None,
            };
            if self.date == DateStyle::MonthDayYear {
                (year, n(0)?, n(1)?)
            } else {
                (year, n(1)?, n(0)?)
            }
        } else {
            return None;
        };
        date_serial(year, month, day)
    }

    pub fn parse_value(&self, s: &str) -> Option<f64> {
        self.parse_number(s)
            .or_else(|| self.parse_date_time(s).map(|(serial, _)| serial))
    }

    pub fn render(&self, value: &CellValue, code: &str) -> Rendered {
        match self.date {
            DateStyle::DayMonthYear => numfmt::render(value, code, self),
            DateStyle::MonthDayYear => {
                let code = match code {
                    numfmt::DATE => numfmt::US_DATE,
                    numfmt::DATE_TIME => numfmt::US_DATE_TIME,
                    other => other,
                };
                numfmt::render(value, code, self)
            }
            DateStyle::Iso => Rendered {
                text: match value {
                    CellValue::Number(n) if numfmt::kind(code) == Kind::Date => {
                        iso(*n).unwrap_or_else(|| self.format_number(*n))
                    }
                    other => self.display(other),
                },
                color: None,
            },
        }
    }

    pub fn format_value(&self, value: &CellValue, code: &str) -> String {
        self.render(value, code).text
    }

    pub fn edit_text(&self, value: &CellValue, code: &str) -> String {
        match value {
            CellValue::Number(n) if numfmt::kind(code) == Kind::Date => {
                let code = if n.fract() == 0.0 {
                    numfmt::DATE
                } else if *n < 1.0 {
                    numfmt::TIME_SECONDS
                } else {
                    numfmt::DATE_TIME
                };
                self.format_value(value, code)
            }
            CellValue::Number(n) if numfmt::kind(code) == Kind::Number && code.contains('%') => {
                format!("{}%", self.format_number(round_significant(n * 100.0)))
            }
            other => self.display(other),
        }
    }

    pub fn display(&self, value: &CellValue) -> String {
        match value {
            CellValue::Empty => String::new(),
            CellValue::Number(n) => self.format_number(*n),
            CellValue::Text(s) => s.clone(),
            CellValue::Bool(b) => self.bool_name(*b).to_owned(),
            CellValue::Error(e) => self.error_name(*e).to_owned(),
        }
    }
}

fn iso(serial: f64) -> Option<String> {
    let dt = from_serial(serial)?;
    let date = format!("{:04}-{:02}-{:02}", dt.year, dt.month, dt.day);
    let time = format!("{:02}:{:02}:{:02}", dt.hour, dt.minute, dt.second);
    Some(if serial.fract() == 0.0 {
        date
    } else if serial < 1.0 {
        time
    } else {
        format!("{date} {time}")
    })
}

fn parse_time(s: &str) -> Option<(f64, bool)> {
    let parts: Vec<&str> = s.split(':').collect();
    let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    if !(2..=3).contains(&parts.len())
        || !digits(parts[0])
        || parts[0].len() > 2
        || !parts[1..].iter().all(|p| digits(p) && p.len() == 2)
    {
        return None;
    }
    let n = |i: usize| parts.get(i).map_or(Some(0), |p| p.parse::<u32>().ok());
    Some((time_fraction(n(0)?, n(1)?, n(2)?)?, parts.len() == 3))
}

pub fn fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'i' | 'ı' | 'İ' => 'I',
            c => c,
        })
        .flat_map(char::to_uppercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_turkish_numbers() {
        assert_eq!(TR.parse_number("1234,56"), Some(1234.56));
        assert_eq!(TR.parse_number("1.234,56"), Some(1234.56));
        assert_eq!(TR.parse_number("-1.234.567"), Some(-1_234_567.0));
        assert_eq!(TR.parse_number(",5"), Some(0.5));
        assert_eq!(TR.parse_number("1,5E+3"), Some(1500.0));
        assert_eq!(TR.parse_number("50%"), Some(0.5));
        assert_eq!(TR.parse_number("12,5%"), Some(0.125));
    }

    #[test]
    fn rejects_malformed_turkish_numbers() {
        for s in [
            "1.5", "1234.56", "1.23", "12.3456", "1,2,3", "1,234.5", ".123", "", "%", "abc",
        ] {
            assert_eq!(TR.parse_number(s), None, "{s}");
        }
    }

    #[test]
    fn parses_english_numbers() {
        assert_eq!(EN.parse_number("1,234.5"), Some(1234.5));
        assert_eq!(EN.parse_number("1234.5"), Some(1234.5));
        assert_eq!(EN.parse_number("1,5"), None);
    }

    #[test]
    fn formats_with_decimal_comma() {
        assert_eq!(TR.format_number(1234.56), "1234,56");
        assert_eq!(TR.format_number(0.1 + 0.2), "0,3");
        assert_eq!(TR.format_number(1.5e20), "1,5E+20");
        assert_eq!(
            TR.format_number_fit(1.0 / 3.0, 8).as_deref(),
            Some("0,333333")
        );
        assert_eq!(EN.format_number(1234.56), "1234.56");
    }

    fn date(s: &str, locale: &Locale) -> Option<(f64, &'static str)> {
        locale.parse_date_time(s)
    }

    #[test]
    fn parses_turkish_dates_and_times() {
        assert_eq!(date("17.10.2026", &TR), Some((46312.0, numfmt::DATE)));
        assert_eq!(date("1.2.26", &TR), Some((46054.0, numfmt::DATE)));
        assert_eq!(date("17/10/2026", &TR), Some((46312.0, numfmt::DATE)));
        assert_eq!(date("2026-10-17", &TR), Some((46312.0, numfmt::DATE)));
        let (time, code) = date("13:45", &TR).unwrap();
        assert_eq!(code, numfmt::TIME);
        assert!((time - 13.75 / 24.0).abs() < 1e-12);
        assert_eq!(
            date("8:05:30", &TR).map(|d| d.1),
            Some(numfmt::TIME_SECONDS)
        );
        assert_eq!(
            date("17.10.2026 12:00", &TR),
            Some((46312.5, numfmt::DATE_TIME))
        );
    }

    #[test]
    fn rejects_non_dates() {
        for s in [
            "1.5",
            "31.02.2026",
            "17.10",
            "1.234.567",
            "12:3",
            "25:00",
            "abc",
            "",
            "17.10.26.1",
        ] {
            assert_eq!(date(s, &TR), None, "{s}");
        }
        assert_eq!(date("17.10.2026", &EN), None);
        assert_eq!(date("2026-10-17", &EN), Some((46312.0, numfmt::DATE)));
        assert_eq!(date("10/17/2026", &EN_US), Some((46312.0, numfmt::DATE)));
        assert_eq!(date("17/10/2026", &EN_US), None);
    }

    #[test]
    fn formats_dates_per_locale() {
        let serial = 46312.0 + 13.0 / 24.0 + 5.0 / 1440.0;
        let date = CellValue::Number(serial);
        assert_eq!(TR.format_value(&date, numfmt::DATE), "17.10.2026");
        assert_eq!(TR.format_value(&date, numfmt::TIME), "13:05");
        assert_eq!(
            TR.format_value(&date, numfmt::DATE_TIME),
            "17.10.2026 13:05"
        );
        assert_eq!(
            EN.format_value(&date, numfmt::DATE_TIME),
            "2026-10-17 13:05:00"
        );
        assert_eq!(EN_US.format_value(&date, numfmt::DATE), "10/17/2026");
        assert_eq!(
            EN_US.format_value(&date, numfmt::DATE_TIME),
            "10/17/2026 13:05"
        );
        assert_eq!(
            EN_US.format_value(&CellValue::Number(48200.0), "$#,##0"),
            "$48,200"
        );
        assert_eq!(
            EN.format_value(&date, numfmt::CURRENCY),
            EN.format_number(serial)
        );
        assert_eq!(TR.format_value(&date, "General"), TR.format_number(serial));
    }

    #[test]
    fn edit_text_shows_raw_values() {
        let n = CellValue::Number(1234.5);
        assert_eq!(TR.edit_text(&n, numfmt::CURRENCY), "1234,5");
        assert_eq!(
            TR.edit_text(&CellValue::Number(0.125), numfmt::PERCENT),
            "12,5%"
        );
        assert_eq!(
            TR.edit_text(&CellValue::Number(46312.5), numfmt::DATE),
            "17.10.2026 12:00"
        );
        assert_eq!(
            TR.edit_text(&CellValue::Number(0.5), numfmt::TIME),
            "12:00:00"
        );
        assert_eq!(
            EN.edit_text(&CellValue::Number(46312.0), numfmt::DATE),
            "2026-10-17"
        );
    }

    #[test]
    fn booleans_and_errors_are_localized() {
        assert_eq!(TR.display(&CellValue::Bool(true)), "DOĞRU");
        assert_eq!(TR.display(&CellValue::Bool(false)), "YANLIŞ");
        assert_eq!(TR.display(&CellValue::Error(CellError::Div0)), "#SAYI/0!");
        assert_eq!(TR.display(&CellValue::Error(CellError::NA)), "#YOK");
        assert_eq!(TR.parse_bool("doğru"), Some(true));
        assert_eq!(TR.parse_bool("yanlış"), Some(false));
        assert_eq!(TR.parse_bool("TRUE"), None);
        assert_eq!(TR.parse_error("#değer!"), Some(CellError::Value));
        assert_eq!(EN.parse_error("#n/a"), Some(CellError::NA));
    }

    #[test]
    fn function_names_follow_turkish_case_rules() {
        assert_eq!(TR.canonical_function("topla"), "SUM");
        assert_eq!(TR.canonical_function("eğer"), "IF");
        assert_eq!(TR.canonical_function("min"), "MIN");
        assert_eq!(TR.canonical_function("MİN"), "MIN");
        assert_eq!(TR.canonical_function("düşeyara"), "VLOOKUP");
        assert_eq!(TR.canonical_function("sum"), "SUM");
        assert_eq!(TR.canonical_function("bilinmeyen"), "BILINMEYEN");
        assert_eq!(TR.function_name("XLOOKUP"), "ÇAPRAZARA");
        assert_eq!(EN.function_name("SUM"), "SUM");
    }
}
