use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;

use crate::{CellAddr, Color};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    Bmp,
}

impl ImageFormat {
    pub fn detect(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [0x89, b'P', b'N', b'G', ..] => Some(Self::Png),
            [0xFF, 0xD8, ..] => Some(Self::Jpeg),
            [b'G', b'I', b'F', ..] => Some(Self::Gif),
            [b'B', b'M', ..] => Some(Self::Bmp),
            _ => None,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Gif => "gif",
            Self::Bmp => "bmp",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub data: Arc<[u8]>,
    pub format: ImageFormat,
    pub anchor: CellAddr,
    pub offset: (u32, u32),
    pub size: (u32, u32),
}

impl Picture {
    pub fn new(data: Vec<u8>, anchor: CellAddr, size: (u32, u32)) -> Option<Self> {
        let format = ImageFormat::detect(&data)?;
        Some(Self {
            data: data.into(),
            format,
            anchor,
            offset: (0, 0),
            size,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    TextBox,
    Line,
    Arrow,
    DoubleArrow,
    Rectangle,
    RoundedRectangle,
    Ellipse,
    Triangle,
    RightTriangle,
    Parallelogram,
    Trapezoid,
    Diamond,
    Pentagon,
    Hexagon,
    Octagon,
    Plus,
    RightArrow,
    LeftArrow,
    UpArrow,
    DownArrow,
    LeftRightArrow,
    Chevron,
    Star4,
    Star5,
    Star6,
}

impl ShapeKind {
    pub const ALL: [Self; 25] = [
        Self::TextBox,
        Self::Line,
        Self::Arrow,
        Self::DoubleArrow,
        Self::Rectangle,
        Self::RoundedRectangle,
        Self::Ellipse,
        Self::Triangle,
        Self::RightTriangle,
        Self::Parallelogram,
        Self::Trapezoid,
        Self::Diamond,
        Self::Pentagon,
        Self::Hexagon,
        Self::Octagon,
        Self::Plus,
        Self::RightArrow,
        Self::LeftArrow,
        Self::UpArrow,
        Self::DownArrow,
        Self::LeftRightArrow,
        Self::Chevron,
        Self::Star4,
        Self::Star5,
        Self::Star6,
    ];

    pub fn preset(self) -> &'static str {
        match self {
            Self::TextBox | Self::Rectangle => "rect",
            Self::Line | Self::Arrow | Self::DoubleArrow => "line",
            Self::RoundedRectangle => "roundRect",
            Self::Ellipse => "ellipse",
            Self::Triangle => "triangle",
            Self::RightTriangle => "rtTriangle",
            Self::Parallelogram => "parallelogram",
            Self::Trapezoid => "trapezoid",
            Self::Diamond => "diamond",
            Self::Pentagon => "pentagon",
            Self::Hexagon => "hexagon",
            Self::Octagon => "octagon",
            Self::Plus => "plus",
            Self::RightArrow => "rightArrow",
            Self::LeftArrow => "leftArrow",
            Self::UpArrow => "upArrow",
            Self::DownArrow => "downArrow",
            Self::LeftRightArrow => "leftRightArrow",
            Self::Chevron => "chevron",
            Self::Star4 => "star4",
            Self::Star5 => "star5",
            Self::Star6 => "star6",
        }
    }

    pub fn from_preset(preset: &str, text_box: bool, head: bool, tail: bool) -> Option<Self> {
        match preset {
            "rect" if text_box => Some(Self::TextBox),
            "line" | "straightConnector1" => Some(match (head, tail) {
                (true, true) => Self::DoubleArrow,
                (false, false) => Self::Line,
                _ => Self::Arrow,
            }),
            "flowChartProcess" => Some(Self::Rectangle),
            "flowChartDecision" => Some(Self::Diamond),
            "flowChartAlternateProcess" => Some(Self::RoundedRectangle),
            "flowChartConnector" => Some(Self::Ellipse),
            _ => Self::ALL
                .into_iter()
                .find(|kind| kind.preset() == preset && !kind.is_line() && *kind != Self::TextBox),
        }
    }

    pub fn is_line(self) -> bool {
        matches!(self, Self::Line | Self::Arrow | Self::DoubleArrow)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAnchor {
    #[default]
    Top,
    Middle,
    Bottom,
}

pub const ACCENT: Color = Color(0x4472C4);
pub const ACCENT_DARK: Color = Color(0x2F528F);
pub const TEXT_BOX_LINE: Color = Color(0xBFBFBF);
pub const DEFAULT_SHAPE_SIZE: (u32, u32) = (96, 96);
pub const DEFAULT_TEXT_BOX_SIZE: (u32, u32) = (192, 96);

#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub kind: ShapeKind,
    pub anchor: CellAddr,
    pub offset: (u32, u32),
    pub size: (u32, u32),
    pub flip_h: bool,
    pub flip_v: bool,
    pub fill: Option<Color>,
    pub line: Option<Color>,
    pub line_width: u16,
    pub text: String,
    pub font_size: u16,
    pub bold: bool,
    pub italic: bool,
    pub color: Color,
    pub align: TextAlign,
    pub text_anchor: TextAnchor,
}

impl Shape {
    pub fn new(kind: ShapeKind, anchor: CellAddr) -> Self {
        let text_box = kind == ShapeKind::TextBox;
        let plain = text_box || kind.is_line();
        let size = if text_box {
            DEFAULT_TEXT_BOX_SIZE
        } else {
            DEFAULT_SHAPE_SIZE
        };
        Self {
            kind,
            anchor,
            offset: (0, 0),
            size,
            flip_h: false,
            flip_v: false,
            fill: if text_box {
                Some(Color(0xFFFFFF))
            } else if kind.is_line() {
                None
            } else {
                Some(ACCENT)
            },
            line: Some(match kind {
                ShapeKind::TextBox => TEXT_BOX_LINE,
                _ if kind.is_line() => ACCENT,
                _ => ACCENT_DARK,
            }),
            line_width: if kind.is_line() || text_box { 75 } else { 100 },
            text: String::new(),
            font_size: 110,
            bold: false,
            italic: false,
            color: if plain {
                Color(0x000000)
            } else {
                Color(0xFFFFFF)
            },
            align: if plain {
                TextAlign::Left
            } else {
                TextAlign::Center
            },
            text_anchor: if plain {
                TextAnchor::Top
            } else {
                TextAnchor::Middle
            },
        }
    }

    pub fn points(&self, width: f64, height: f64) -> Vec<(f64, f64)> {
        outline(self.kind, width, height)
            .into_iter()
            .map(|(x, y)| {
                (
                    if self.flip_h { width - x } else { x },
                    if self.flip_v { height - y } else { y },
                )
            })
            .collect()
    }
}

fn arc(cx: f64, cy: f64, rx: f64, ry: f64, from: f64, to: f64, steps: usize) -> Vec<(f64, f64)> {
    (0..=steps)
        .map(|i| {
            let a = from + (to - from) * i as f64 / steps as f64;
            (cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect()
}

fn fit(points: Vec<(f64, f64)>, w: f64, h: f64) -> Vec<(f64, f64)> {
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (x, y) in &points {
        min_x = min_x.min(*x);
        min_y = min_y.min(*y);
        max_x = max_x.max(*x);
        max_y = max_y.max(*y);
    }
    let sx = w / (max_x - min_x).max(f64::EPSILON);
    let sy = h / (max_y - min_y).max(f64::EPSILON);
    points
        .into_iter()
        .map(|(x, y)| ((x - min_x) * sx, (y - min_y) * sy))
        .collect()
}

fn star(tips: usize, inner: f64, w: f64, h: f64) -> Vec<(f64, f64)> {
    let points = (0..tips * 2)
        .map(|i| {
            let a = -FRAC_PI_2 + PI * i as f64 / tips as f64;
            let r = if i % 2 == 0 { 1.0 } else { inner };
            (r * a.cos(), r * a.sin())
        })
        .collect();
    fit(points, w, h)
}

fn arrow(length: f64, thickness: f64) -> Vec<(f64, f64)> {
    let head = (thickness / 2.0).min(length);
    let (top, bottom) = (thickness / 4.0, thickness * 3.0 / 4.0);
    vec![
        (0.0, top),
        (length - head, top),
        (length - head, 0.0),
        (length, thickness / 2.0),
        (length - head, thickness),
        (length - head, bottom),
        (0.0, bottom),
    ]
}

pub fn outline(kind: ShapeKind, w: f64, h: f64) -> Vec<(f64, f64)> {
    let ss = w.min(h);
    match kind {
        ShapeKind::Line | ShapeKind::Arrow | ShapeKind::DoubleArrow => vec![(0.0, 0.0), (w, h)],
        ShapeKind::TextBox | ShapeKind::Rectangle => vec![(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)],
        ShapeKind::RoundedRectangle => {
            let r = ss * 0.16667;
            let mut points = arc(w - r, r, r, r, -FRAC_PI_2, 0.0, 8);
            points.extend(arc(w - r, h - r, r, r, 0.0, FRAC_PI_2, 8));
            points.extend(arc(r, h - r, r, r, FRAC_PI_2, PI, 8));
            points.extend(arc(r, r, r, r, PI, PI + FRAC_PI_2, 8));
            points
        }
        ShapeKind::Ellipse => {
            let mut points = arc(w / 2.0, h / 2.0, w / 2.0, h / 2.0, 0.0, TAU, 64);
            points.pop();
            points
        }
        ShapeKind::Triangle => vec![(w / 2.0, 0.0), (w, h), (0.0, h)],
        ShapeKind::RightTriangle => vec![(0.0, 0.0), (w, h), (0.0, h)],
        ShapeKind::Parallelogram => {
            let a = ss * 0.25;
            vec![(a, 0.0), (w, 0.0), (w - a, h), (0.0, h)]
        }
        ShapeKind::Trapezoid => {
            let a = ss * 0.25;
            vec![(a, 0.0), (w - a, 0.0), (w, h), (0.0, h)]
        }
        ShapeKind::Diamond => vec![(w / 2.0, 0.0), (w, h / 2.0), (w / 2.0, h), (0.0, h / 2.0)],
        ShapeKind::Pentagon => {
            let points = (0..5)
                .map(|i| {
                    let a = -FRAC_PI_2 + TAU * i as f64 / 5.0;
                    (a.cos(), a.sin())
                })
                .collect();
            fit(points, w, h)
        }
        ShapeKind::Hexagon => {
            let a = ss * 0.25;
            vec![
                (a, 0.0),
                (w - a, 0.0),
                (w, h / 2.0),
                (w - a, h),
                (a, h),
                (0.0, h / 2.0),
            ]
        }
        ShapeKind::Octagon => {
            let a = ss * 0.29289;
            vec![
                (a, 0.0),
                (w - a, 0.0),
                (w, a),
                (w, h - a),
                (w - a, h),
                (a, h),
                (0.0, h - a),
                (0.0, a),
            ]
        }
        ShapeKind::Plus => {
            let a = ss * 0.25;
            vec![
                (a, 0.0),
                (w - a, 0.0),
                (w - a, a),
                (w, a),
                (w, h - a),
                (w - a, h - a),
                (w - a, h),
                (a, h),
                (a, h - a),
                (0.0, h - a),
                (0.0, a),
                (a, a),
            ]
        }
        ShapeKind::RightArrow => arrow(w, h),
        ShapeKind::LeftArrow => arrow(w, h).into_iter().map(|(x, y)| (w - x, y)).collect(),
        ShapeKind::DownArrow => arrow(h, w).into_iter().map(|(x, y)| (y, x)).collect(),
        ShapeKind::UpArrow => arrow(h, w).into_iter().map(|(x, y)| (y, h - x)).collect(),
        ShapeKind::LeftRightArrow => {
            let head = (h / 2.0).min(w / 2.0);
            let (top, bottom) = (h / 4.0, h * 3.0 / 4.0);
            vec![
                (0.0, h / 2.0),
                (head, 0.0),
                (head, top),
                (w - head, top),
                (w - head, 0.0),
                (w, h / 2.0),
                (w - head, h),
                (w - head, bottom),
                (head, bottom),
                (head, h),
            ]
        }
        ShapeKind::Chevron => {
            let a = (ss * 0.5).min(w);
            vec![
                (0.0, 0.0),
                (w - a, 0.0),
                (w, h / 2.0),
                (w - a, h),
                (0.0, h),
                (a, h / 2.0),
            ]
        }
        ShapeKind::Star4 => star(4, 0.25, w, h),
        ShapeKind::Star5 => star(5, 0.382, w, h),
        ShapeKind::Star6 => star(6, 0.577, w, h),
    }
}

fn area(points: &[(f64, f64)]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f64>()
        / 2.0
}

fn cross(o: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

fn inside(p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    cross(a, b, p) > 0.0 && cross(b, c, p) > 0.0 && cross(c, a, p) > 0.0
}

pub fn triangulate(points: &[(f64, f64)]) -> Vec<[usize; 3]> {
    let mut order: Vec<usize> = (0..points.len()).collect();
    if area(points) < 0.0 {
        order.reverse();
    }
    let mut out = Vec::new();
    let mut guard = 0;
    while order.len() > 3 && guard < points.len() * points.len() {
        guard += 1;
        let n = order.len();
        let ear = (0..n).find(|&i| {
            let (a, b, c) = (order[(i + n - 1) % n], order[i], order[(i + 1) % n]);
            cross(points[a], points[b], points[c]) > 0.0
                && order
                    .iter()
                    .filter(|&&j| j != a && j != b && j != c)
                    .all(|&j| !inside(points[j], points[a], points[b], points[c]))
        });
        let i = ear.unwrap_or(0);
        out.push([order[(i + n - 1) % n], order[i], order[(i + 1) % n]]);
        order.remove(i);
    }
    if order.len() == 3 {
        out.push([order[0], order[1], order[2]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn covered(points: &[(f64, f64)]) -> f64 {
        triangulate(points)
            .iter()
            .map(|[a, b, c]| area(&[points[*a], points[*b], points[*c]]).abs())
            .sum()
    }

    #[test]
    fn every_closed_shape_triangulates_to_its_area() {
        for kind in ShapeKind::ALL.into_iter().filter(|k| !k.is_line()) {
            let points = outline(kind, 120.0, 80.0);
            let expected = area(&points).abs();
            assert!(expected > 0.0, "{kind:?}");
            assert_eq!(triangulate(&points).len(), points.len() - 2, "{kind:?}");
            assert!((covered(&points) - expected).abs() < 1e-6, "{kind:?}");
            for (x, y) in points {
                assert!((-1e-9..=120.0 + 1e-9).contains(&x), "{kind:?}");
                assert!((-1e-9..=80.0 + 1e-9).contains(&y), "{kind:?}");
            }
        }
    }

    #[test]
    fn arrows_point_the_right_way() {
        let tip = |kind| {
            outline(kind, 100.0, 60.0)
                .into_iter()
                .find(|&(x, y)| {
                    (x == 0.0 || x == 100.0 || y == 0.0 || y == 60.0) && (x == 50.0 || y == 30.0)
                })
                .expect("tip")
        };
        assert_eq!(tip(ShapeKind::RightArrow), (100.0, 30.0));
        assert_eq!(tip(ShapeKind::LeftArrow), (0.0, 30.0));
        assert_eq!(tip(ShapeKind::UpArrow), (50.0, 0.0));
        assert_eq!(tip(ShapeKind::DownArrow), (50.0, 60.0));
    }

    #[test]
    fn flips_mirror_lines_and_shapes() {
        let mut line = Shape::new(ShapeKind::Line, CellAddr::new(0, 0).expect("valid"));
        line.flip_v = true;
        assert_eq!(line.points(10.0, 20.0), vec![(0.0, 20.0), (10.0, 0.0)]);
        let mut triangle = Shape::new(ShapeKind::RightTriangle, line.anchor);
        triangle.flip_h = true;
        assert_eq!(triangle.points(10.0, 20.0)[0], (10.0, 0.0));
    }

    #[test]
    fn presets_round_trip() {
        for kind in ShapeKind::ALL {
            let (head, tail) = match kind {
                ShapeKind::Arrow => (false, true),
                ShapeKind::DoubleArrow => (true, true),
                _ => (false, false),
            };
            let text_box = kind == ShapeKind::TextBox;
            assert_eq!(
                ShapeKind::from_preset(kind.preset(), text_box, head, tail),
                Some(kind)
            );
        }
        assert_eq!(ImageFormat::detect(b"\x89PNG\r\n"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::detect(b"nope"), None);
    }
}
