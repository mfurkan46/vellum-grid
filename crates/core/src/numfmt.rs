use crate::date::from_serial;
use crate::style::{Color, GENERAL};
use crate::value::round_decimal;
use crate::{CellValue, Locale};

pub const DATE: &str = "dd.mm.yyyy";
pub const LONG_DATE: &str = "d mmmm yyyy dddd";
pub const TIME: &str = "hh:mm";
pub const TIME_SECONDS: &str = "hh:mm:ss";
pub const DATE_TIME: &str = "dd.mm.yyyy hh:mm";
pub const US_DATE: &str = "m/d/yyyy";
pub const US_DATE_TIME: &str = "m/d/yyyy h:mm";
pub const NUMBER: &str = "0.00";
pub const THOUSANDS: &str = "#,##0.00";
pub const CURRENCY: &str = "#,##0.00 \"₺\"";
pub const PERCENT: &str = "0%";
pub const PERCENT_DECIMAL: &str = "0.00%";
pub const SCIENTIFIC: &str = "0.00E+00";
pub const TEXT: &str = "@";

const OVERFLOW: &str = "########";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    General,
    Number,
    Date,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub text: String,
    pub color: Option<Color>,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Literal(String),
    Zero,
    Hash,
    Question,
    Point,
    Comma,
    Percent,
    Exponent(bool),
    Year(usize),
    Month(usize),
    Day(usize),
    Hour(usize),
    Minute(usize),
    Second(usize),
    AmPm,
    Text,
    General,
    Slash,
    Denominator(u32),
}

#[derive(Debug, Clone, PartialEq)]
struct Section {
    tokens: Vec<Token>,
    color: Option<Color>,
}

impl Section {
    fn is_date(&self) -> bool {
        self.tokens.iter().any(|t| {
            matches!(
                t,
                Token::Year(_)
                    | Token::Month(_)
                    | Token::Day(_)
                    | Token::Hour(_)
                    | Token::Minute(_)
                    | Token::Second(_)
            )
        })
    }

    fn has_digits(&self) -> bool {
        self.tokens.iter().any(|t| {
            matches!(
                t,
                Token::Zero | Token::Hash | Token::Question | Token::General
            )
        })
    }
}

pub fn kind(code: &str) -> Kind {
    let sections = parse(code);
    let first = &sections[0];
    if first.tokens.iter().all(|t| matches!(t, Token::General)) {
        Kind::General
    } else if first.is_date() {
        Kind::Date
    } else if first.tokens.contains(&Token::Text) && !first.has_digits() {
        Kind::Text
    } else {
        Kind::Number
    }
}

pub fn render(value: &CellValue, code: &str, locale: &Locale) -> Rendered {
    let sections = parse(code);
    match value {
        CellValue::Number(n) => render_number(*n, &sections, locale),
        CellValue::Text(text) => {
            let section = sections
                .get(3)
                .or_else(|| sections.iter().find(|s| s.tokens.contains(&Token::Text)));
            match section {
                Some(section) => Rendered {
                    text: section
                        .tokens
                        .iter()
                        .map(|t| match t {
                            Token::Literal(s) => s.clone(),
                            Token::Text => text.clone(),
                            _ => String::new(),
                        })
                        .collect(),
                    color: section.color,
                },
                None => Rendered {
                    text: text.clone(),
                    color: None,
                },
            }
        }
        other => Rendered {
            text: locale.display(other),
            color: None,
        },
    }
}

fn render_number(n: f64, sections: &[Section], locale: &Locale) -> Rendered {
    let (section, value, signed) = match sections.len() {
        1 => (&sections[0], n, true),
        _ if n > 0.0 => (&sections[0], n, false),
        _ if n < 0.0 => (&sections[1], -n, false),
        2 => (&sections[0], n, false),
        _ => (&sections[2], n, false),
    };
    let text = if section.tokens.iter().all(|t| matches!(t, Token::General)) {
        locale.format_number(value)
    } else if section.is_date() {
        render_date(value, &section.tokens, locale).unwrap_or_else(|| OVERFLOW.to_owned())
    } else if section.tokens.contains(&Token::Slash) {
        render_fraction(value, &section.tokens, signed)
    } else {
        render_digits(value, &section.tokens, locale, signed)
    };
    Rendered {
        text,
        color: section.color,
    }
}

fn render_date(serial: f64, tokens: &[Token], locale: &Locale) -> Option<String> {
    let dt = from_serial(serial)?;
    let twelve = tokens.contains(&Token::AmPm);
    let weekday = (serial.floor() as i64 - 1).rem_euclid(7) as usize;
    let month = dt.month as usize - 1;
    let pad = |value: u32, width: usize| format!("{value:0width$}");
    let mut out = String::new();
    for token in tokens {
        match token {
            Token::Literal(s) => out.push_str(s),
            Token::Year(2) => out.push_str(&pad((dt.year % 100) as u32, 2)),
            Token::Year(_) => out.push_str(&dt.year.to_string()),
            Token::Month(1) => out.push_str(&dt.month.to_string()),
            Token::Month(2) => out.push_str(&pad(dt.month, 2)),
            Token::Month(3) => out.push_str(locale.months_short[month]),
            Token::Month(5) => out.extend(locale.months[month].chars().take(1)),
            Token::Month(_) => out.push_str(locale.months[month]),
            Token::Day(1) => out.push_str(&dt.day.to_string()),
            Token::Day(2) => out.push_str(&pad(dt.day, 2)),
            Token::Day(3) => out.push_str(locale.days_short[weekday]),
            Token::Day(_) => out.push_str(locale.days[weekday]),
            Token::Hour(width) => {
                let hour = if twelve {
                    (dt.hour + 11) % 12 + 1
                } else {
                    dt.hour
                };
                out.push_str(&pad(hour, (*width).min(2)));
            }
            Token::Minute(width) => out.push_str(&pad(dt.minute, (*width).min(2))),
            Token::Second(width) => out.push_str(&pad(dt.second, (*width).min(2))),
            Token::AmPm => out.push_str(if dt.hour < 12 { locale.am } else { locale.pm }),
            Token::Point => out.push('.'),
            _ => {}
        }
    }
    Some(out)
}

fn render_digits(value: f64, tokens: &[Token], locale: &Locale, signed: bool) -> String {
    let placeholder = |t: &Token| matches!(t, Token::Zero | Token::Hash | Token::Question);
    let point = tokens.iter().position(|t| *t == Token::Point);
    let exponent = tokens.iter().position(|t| matches!(t, Token::Exponent(_)));
    let mantissa_end = exponent.unwrap_or(tokens.len());
    let last_digit = tokens[..mantissa_end].iter().rposition(placeholder);
    let int_end = point
        .unwrap_or(last_digit.map_or(0, |i| i + 1))
        .min(mantissa_end);
    let int_tokens: Vec<&Token> = tokens[..int_end]
        .iter()
        .filter(|t| placeholder(t))
        .collect();
    let frac_tokens: Vec<&Token> = point.map_or(Vec::new(), |p| {
        tokens[p + 1..mantissa_end]
            .iter()
            .filter(|t| placeholder(t))
            .collect()
    });
    let grouping = tokens[..int_end]
        .windows(2)
        .any(|w| w[0] == Token::Comma || w[1] == Token::Comma)
        && tokens[..int_end].iter().any(placeholder);
    let scaling = last_digit.map_or(0, |last| {
        tokens[last + 1..mantissa_end]
            .iter()
            .take_while(|t| **t == Token::Comma)
            .count()
    });
    let percents = tokens.iter().filter(|t| **t == Token::Percent).count();

    let mut magnitude = value.abs() * 100f64.powi(percents as i32) / 1000f64.powi(scaling as i32);
    let mut exponent_value = 0;
    if exponent.is_some() && magnitude != 0.0 {
        let shift = int_tokens.len().max(1) as i32 - 1;
        exponent_value = magnitude.log10().floor() as i32 - shift;
        magnitude /= 10f64.powi(exponent_value);
        if round_decimal(magnitude, frac_tokens.len() as i32) >= 10f64.powi(shift + 1) {
            magnitude /= 10.0;
            exponent_value += 1;
        }
    }
    let rounded = round_decimal(magnitude, frac_tokens.len() as i32);
    let fixed = format!("{rounded:.*}", frac_tokens.len());
    let (int_digits, frac_digits) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let min_int = int_tokens.iter().filter(|t| ***t == Token::Zero).count();
    let int_digits = int_digits.trim_start_matches('0');
    let mut int_text = format!("{int_digits:0>min_int$}");
    if grouping {
        int_text = group(&int_text, locale.group);
    }
    let mut frac_text: Vec<char> = frac_digits.chars().collect();
    for (i, token) in frac_tokens.iter().enumerate().rev() {
        if frac_text[i] != '0' {
            break;
        }
        match token {
            Token::Hash => {
                frac_text.pop();
            }
            Token::Question => frac_text[i] = ' ',
            _ => break,
        }
    }
    let negative = signed && value < 0.0 && rounded != 0.0;

    let mut out = String::new();
    let mut number_written = false;
    for (i, token) in tokens.iter().enumerate() {
        match token {
            Token::Literal(s) => out.push_str(s),
            Token::Percent => out.push('%'),
            Token::Text | Token::General => {}
            t if (placeholder(t) || *t == Token::Point) && i < mantissa_end => {
                if !number_written {
                    number_written = true;
                    if negative {
                        out.push('-');
                    }
                    out.push_str(&int_text);
                    if point.is_some() {
                        out.push(locale.decimal);
                        out.extend(frac_text.iter());
                    }
                }
            }
            Token::Exponent(plus) => {
                let sign = if exponent_value < 0 {
                    "-"
                } else if *plus {
                    "+"
                } else {
                    ""
                };
                let width = tokens[i + 1..].iter().filter(|t| placeholder(t)).count();
                out.push('E');
                out.push_str(sign);
                out.push_str(&format!("{:0width$}", exponent_value.abs()));
                break;
            }
            _ => {}
        }
    }
    if !number_written && negative {
        out.insert(0, '-');
    }
    out
}

fn closest_fraction(value: f64, max_den: u32) -> (u64, u64) {
    let mut best = (value.round() as u64, 1u64, (value - value.round()).abs());
    for den in 2..=u64::from(max_den.max(1)) {
        let num = (value * den as f64).round();
        let error = (value - num / den as f64).abs();
        if error < best.2 - 1e-12 {
            best = (num as u64, den, error);
        }
    }
    (best.0, best.1)
}

fn render_fraction(value: f64, tokens: &[Token], signed: bool) -> String {
    let placeholder = |t: &Token| matches!(t, Token::Zero | Token::Hash | Token::Question);
    let slash = tokens
        .iter()
        .position(|t| *t == Token::Slash)
        .unwrap_or(tokens.len());
    let num_start = tokens[..slash]
        .iter()
        .rposition(|t| !placeholder(t))
        .map_or(0, |i| i + 1);
    let int_end = tokens[..num_start]
        .iter()
        .rposition(placeholder)
        .map(|i| i + 1);
    let den_tokens: Vec<&Token> = tokens[slash + 1..]
        .iter()
        .take_while(|t| placeholder(t) || matches!(t, Token::Denominator(_)))
        .collect();
    let den_end = slash + 1 + den_tokens.len();
    let magnitude = value.abs();
    let (mut whole, part) = match int_end {
        Some(_) => (magnitude.trunc() as u64, magnitude.fract()),
        None => (0, magnitude),
    };
    let (mut num, den) = match den_tokens.first() {
        Some(Token::Denominator(den)) => ((part * f64::from(*den)).round() as u64, u64::from(*den)),
        _ => {
            let digits = den_tokens.len().clamp(1, 9) as u32;
            closest_fraction(part, 10u32.pow(digits) - 1)
        }
    };
    if int_end.is_some() && num >= den && den > 0 {
        whole += num / den;
        num %= den;
    }
    let pad = |text: String, slots: &[&Token], left: bool| {
        let width = slots.len().max(text.len());
        let fill = slots.contains(&&Token::Question);
        if fill && left {
            format!("{text:<width$}")
        } else if fill {
            format!("{text:>width$}")
        } else {
            text
        }
    };
    let num_slots: Vec<&Token> = tokens[num_start..slash].iter().collect();
    let int_slots: Vec<&Token> = int_end.map_or(Vec::new(), |end| {
        tokens[..end].iter().filter(|t| placeholder(t)).collect()
    });
    let blank_fraction = int_end.is_some() && num == 0;
    let int_text = match int_end {
        Some(_) if whole == 0 && !blank_fraction => {
            let zeros = int_slots.contains(&&Token::Zero);
            if zeros { "0".to_owned() } else { String::new() }
        }
        Some(_) => whole.to_string(),
        None => String::new(),
    };
    let fraction = |text: String, slots: &[&Token], left: bool| {
        if blank_fraction {
            " ".repeat(slots.len().max(text.len()))
        } else {
            pad(text, slots, left)
        }
    };
    let den_text = match den_tokens.first() {
        Some(Token::Denominator(_)) => {
            if blank_fraction {
                " ".repeat(den.to_string().len())
            } else {
                den.to_string()
            }
        }
        _ => fraction(den.to_string(), &den_tokens, true),
    };
    let mut out = String::new();
    if signed && value < 0.0 && (whole > 0 || num > 0) {
        out.push('-');
    }
    let mut int_written = false;
    for (i, token) in tokens.iter().enumerate() {
        match token {
            Token::Literal(s) if !(blank_fraction && i > num_start && i < den_end) => {
                out.push_str(s)
            }
            t if placeholder(t) && int_end.is_some_and(|end| i < end) => {
                if !int_written {
                    int_written = true;
                    out.push_str(&pad(int_text.clone(), &int_slots, false));
                }
            }
            _ if i == num_start && i < slash => {
                out.push_str(&fraction(num.to_string(), &num_slots, false))
            }
            Token::Slash => out.push(if blank_fraction { ' ' } else { '/' }),
            _ if i == slash + 1 => out.push_str(&den_text),
            Token::Percent => out.push('%'),
            _ => {}
        }
    }
    out
}

fn group(digits: &str, separator: char) -> String {
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(separator);
        }
        out.push(c);
    }
    out
}

fn parse(code: &str) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut current = String::new();
    let mut chars = code.chars().peekable();
    let mut quoted = false;
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                current.push(c);
            }
            '\\' => {
                current.push(c);
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            ';' if !quoted => sections.push(section(&std::mem::take(&mut current))),
            c => current.push(c),
        }
    }
    sections.push(section(&current));
    sections
}

fn color_name(name: &str) -> Option<Color> {
    let color = match name.to_ascii_lowercase().as_str() {
        "black" | "siyah" => 0x000000,
        "red" | "kırmızı" => 0xFF0000,
        "green" | "yeşil" => 0x00FF00,
        "blue" | "mavi" => 0x0000FF,
        "yellow" | "sarı" => 0xFFFF00,
        "magenta" | "eflatun" => 0xFF00FF,
        "cyan" | "camgöbeği" => 0x00FFFF,
        "white" | "beyaz" => 0xFFFFFF,
        _ => return None,
    };
    Some(Color(color))
}

fn section(text: &str) -> Section {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut color = None;
    let mut i = 0;
    let lower = |c: char| c.to_ascii_lowercase();
    let run = |i: usize, c: char| chars[i..].iter().take_while(|x| lower(**x) == c).count();
    while i < chars.len() {
        let c = chars[i];
        match lower(c) {
            '"' => {
                let end = (i + 1..chars.len())
                    .find(|&j| chars[j] == '"')
                    .unwrap_or(chars.len());
                tokens.push(Token::Literal(chars[i + 1..end].iter().collect()));
                i = end + 1;
                continue;
            }
            '\\' => {
                if let Some(next) = chars.get(i + 1) {
                    tokens.push(Token::Literal(next.to_string()));
                }
                i += 2;
                continue;
            }
            '_' => {
                tokens.push(Token::Literal(" ".into()));
                i += 2;
                continue;
            }
            '*' => {
                i += 2;
                continue;
            }
            '[' => {
                let end = (i..chars.len())
                    .find(|&j| chars[j] == ']')
                    .unwrap_or(chars.len());
                let inner: String = chars[i + 1..end].iter().collect();
                if let Some(currency) = inner.strip_prefix('$') {
                    let symbol = currency.split('-').next().unwrap_or_default();
                    tokens.push(Token::Literal(symbol.to_owned()));
                } else if let Some(c) = color_name(&inner) {
                    color = Some(c);
                }
                i = end + 1;
                continue;
            }
            '/' if tokens
                .last()
                .is_some_and(|t| matches!(t, Token::Zero | Token::Hash | Token::Question)) =>
            {
                tokens.push(Token::Slash);
                let digits: String = chars[i + 1..]
                    .iter()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(den) = digits.parse::<u32>()
                    && den > 0
                {
                    tokens.push(Token::Denominator(den));
                    i += 1 + digits.len();
                    continue;
                }
            }
            '0' => tokens.push(Token::Zero),
            '#' => tokens.push(Token::Hash),
            '?' => tokens.push(Token::Question),
            '.' => tokens.push(Token::Point),
            ',' => tokens.push(Token::Comma),
            '%' => tokens.push(Token::Percent),
            '@' => tokens.push(Token::Text),
            'e' if matches!(chars.get(i + 1), Some('+' | '-')) => {
                tokens.push(Token::Exponent(chars[i + 1] == '+'));
                i += 2;
                continue;
            }
            'y' => {
                let n = run(i, 'y');
                tokens.push(Token::Year(if n <= 2 { 2 } else { 4 }));
                i += n;
                continue;
            }
            'd' => {
                let n = run(i, 'd');
                tokens.push(Token::Day(n.min(4)));
                i += n;
                continue;
            }
            'h' => {
                let n = run(i, 'h');
                tokens.push(Token::Hour(n));
                i += n;
                continue;
            }
            's' => {
                let n = run(i, 's');
                tokens.push(Token::Second(n));
                i += n;
                continue;
            }
            'm' => {
                let n = run(i, 'm');
                let after_hour = tokens
                    .iter()
                    .rev()
                    .find(|t| !matches!(t, Token::Literal(_)))
                    .is_some_and(|t| matches!(t, Token::Hour(_)));
                let before_second = chars[i + n..]
                    .iter()
                    .find(|c| c.is_ascii_alphabetic())
                    .is_some_and(|c| lower(*c) == 's');
                tokens.push(if n <= 2 && (after_hour || before_second) {
                    Token::Minute(n)
                } else {
                    Token::Month(n.min(5))
                });
                i += n;
                continue;
            }
            'a' if text[byte_index(&chars, i)..]
                .to_ascii_uppercase()
                .starts_with("AM/PM") =>
            {
                tokens.push(Token::AmPm);
                i += 5;
                continue;
            }
            'g' if text[byte_index(&chars, i)..]
                .to_ascii_lowercase()
                .starts_with("general") =>
            {
                tokens.push(Token::General);
                i += GENERAL.len();
                continue;
            }
            _ => tokens.push(Token::Literal(c.to_string())),
        }
        i += 1;
    }
    if tokens.is_empty() {
        tokens.push(Token::General);
    }
    Section { tokens, color }
}

fn byte_index(chars: &[char], index: usize) -> usize {
    chars[..index].iter().map(|c| c.len_utf8()).sum()
}

pub fn adjust_decimals(code: &str, delta: i32) -> String {
    let base = if code.eq_ignore_ascii_case(GENERAL) {
        "0"
    } else {
        code
    };
    let mut out = String::new();
    let mut done = false;
    let chars: Vec<char> = base.chars().collect();
    let last_digit = chars
        .iter()
        .rposition(|c| matches!(c, '0' | '#'))
        .unwrap_or(chars.len());
    for (i, c) in chars.iter().enumerate() {
        out.push(*c);
        if i == last_digit && !done {
            done = true;
            let has_point = chars[..=i].contains(&'.');
            if delta > 0 {
                if !has_point {
                    out.push('.');
                }
                out.push('0');
            } else if has_point {
                out.pop();
                if out.ends_with('.') {
                    out.pop();
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EN, TR};

    fn tr(n: f64, code: &str) -> String {
        render(&CellValue::Number(n), code, &TR).text
    }

    #[test]
    fn general_and_fixed_decimals() {
        assert_eq!(tr(1234.5, GENERAL), "1234,5");
        assert_eq!(tr(1234.5, "0"), "1235");
        assert_eq!(tr(1234.5, NUMBER), "1234,50");
        assert_eq!(tr(-2.005, NUMBER), "-2,01");
        assert_eq!(tr(0.5, "#.##"), ",5");
        assert_eq!(tr(1.5, "0.0#"), "1,5");
        assert_eq!(tr(1.256, "0.0#"), "1,26");
        assert_eq!(
            render(&CellValue::Number(1234.5), NUMBER, &EN).text,
            "1234.50"
        );
    }

    #[test]
    fn thousands_currency_and_percent() {
        assert_eq!(tr(1_234_567.891, THOUSANDS), "1.234.567,89");
        assert_eq!(tr(1234.5, CURRENCY), "1.234,50 ₺");
        assert_eq!(tr(-1234.5, CURRENCY), "-1.234,50 ₺");
        assert_eq!(tr(0.125, PERCENT), "13%");
        assert_eq!(tr(0.125, PERCENT_DECIMAL), "12,50%");
        assert_eq!(tr(1_234_567.0, "#,##0,"), "1.235");
        assert_eq!(tr(5.0, "#,##0.00 [$₺-41F]"), "5,00 ₺");
        assert_eq!(tr(999.0, "#,##0"), "999");
    }

    #[test]
    fn sections_and_colors() {
        let code = "#,##0.00;[Red]-#,##0.00;\"sıfır\"";
        assert_eq!(tr(5.0, code), "5,00");
        let negative = render(&CellValue::Number(-5.0), code, &TR);
        assert_eq!(negative.text, "-5,00");
        assert_eq!(negative.color, Some(Color(0xFF0000)));
        assert_eq!(tr(0.0, code), "sıfır");
        assert_eq!(tr(-3.0, "0;(0)"), "(3)");
    }

    #[test]
    fn fractions() {
        assert_eq!(tr(1.5, "# ?/?"), "1 1/2");
        assert_eq!(tr(0.75, "# ?/?"), " 3/4");
        assert_eq!(tr(2.0, "# ?/?"), "2    ");
        assert_eq!(tr(0.3333, "# ??/??"), "  1/3 ");
        assert_eq!(tr(std::f64::consts::PI, "# ??/??"), "3 14/99");
        assert_eq!(tr(0.6, "# ?/4"), " 2/4");
        assert_eq!(tr(1.3, "# ??/100"), "1 30/100");
        assert_eq!(tr(-1.25, "# ?/?"), "-1 1/4");
        assert_eq!(tr(1.25, "?/?"), "5/4");
        assert_eq!(kind("# ?/?"), Kind::Number);
    }

    #[test]
    fn scientific() {
        assert_eq!(tr(12345.0, SCIENTIFIC), "1,23E+04");
        assert_eq!(tr(0.000123, SCIENTIFIC), "1,23E-04");
        assert_eq!(tr(9.999, "0.0E+00"), "1,0E+01");
    }

    #[test]
    fn dates_and_times_in_turkish() {
        let serial = 46312.0 + 13.0 / 24.0 + 5.0 / 1440.0;
        assert_eq!(tr(serial, DATE), "17.10.2026");
        assert_eq!(tr(serial, LONG_DATE), "17 Ekim 2026 Cumartesi");
        assert_eq!(tr(serial, "d mmm yy"), "17 Eki 26");
        assert_eq!(tr(serial, "ddd"), "Cmt");
        assert_eq!(tr(serial, TIME), "13:05");
        assert_eq!(tr(serial, "h:mm AM/PM"), "1:05 ÖS");
        assert_eq!(tr(serial, DATE_TIME), "17.10.2026 13:05");
        assert_eq!(tr(serial, "mm:ss"), "05:00");
        assert_eq!(tr(-1.0, DATE), OVERFLOW);
    }

    #[test]
    fn text_sections() {
        let text = CellValue::Text("elma".into());
        assert_eq!(render(&text, TEXT, &TR).text, "elma");
        assert_eq!(render(&text, "0;0;0;\"[\"@\"]\"", &TR).text, "[elma]");
        assert_eq!(render(&text, NUMBER, &TR).text, "elma");
        assert_eq!(render(&CellValue::Bool(true), NUMBER, &TR).text, "DOĞRU");
    }

    #[test]
    fn kinds() {
        assert_eq!(kind(GENERAL), Kind::General);
        assert_eq!(kind(CURRENCY), Kind::Number);
        assert_eq!(kind(DATE), Kind::Date);
        assert_eq!(kind("h:mm"), Kind::Date);
        assert_eq!(kind(TEXT), Kind::Text);
    }

    #[test]
    fn decimals_can_be_added_and_removed() {
        assert_eq!(adjust_decimals(GENERAL, 1), "0.0");
        assert_eq!(adjust_decimals("0", 1), "0.0");
        assert_eq!(adjust_decimals("0.00", 1), "0.000");
        assert_eq!(adjust_decimals("0.00", -1), "0.0");
        assert_eq!(adjust_decimals("0.0", -1), "0");
        assert_eq!(adjust_decimals(CURRENCY, -1), "#,##0.0 \"₺\"");
        assert_eq!(adjust_decimals(PERCENT, 1), "0.0%");
    }
}
