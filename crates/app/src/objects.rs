use std::collections::{HashMap, HashSet};
use std::io::Cursor;

use eframe::egui::epaint::PathShape;
use eframe::egui::text::LayoutJob;
use eframe::egui::{
    Align, Color32, ColorImage, Context, CornerRadius, FontId, Mesh, Painter, Pos2, Rect, Stroke,
    StrokeKind, TextureHandle, TextureOptions, pos2, vec2,
};
use vellum_grid_core::drawing::{
    Picture, Shape, ShapeKind, TextAlign, TextAnchor, outline, triangulate,
};
use vellum_grid_core::{CellAddr, DEFAULT_FONT, Layout};

use crate::app::Spreadsheet;
use crate::dialogs::Dialog;
use crate::{fonts, render, text, theme};

const MAX_TEXTURE_SIDE: u32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Object {
    Picture(usize),
    Chart(usize),
    Shape(usize),
}

pub struct Frame {
    pub anchor: CellAddr,
    pub offset: (u32, u32),
    pub size: (u32, u32),
    pub flip: (bool, bool),
    pub line: bool,
    pub aspect: bool,
}

impl Object {
    pub fn frame(self, layout: &Layout) -> Option<Frame> {
        Some(match self {
            Self::Picture(index) => {
                let p = layout.pictures.get(index)?;
                Frame {
                    anchor: p.anchor,
                    offset: p.offset,
                    size: p.size,
                    flip: (false, false),
                    line: false,
                    aspect: true,
                }
            }
            Self::Chart(index) => {
                let c = layout.charts.get(index)?;
                Frame {
                    anchor: c.anchor,
                    offset: c.offset,
                    size: c.size,
                    flip: (false, false),
                    line: false,
                    aspect: false,
                }
            }
            Self::Shape(index) => {
                let s = layout.shapes.get(index)?;
                Frame {
                    anchor: s.anchor,
                    offset: s.offset,
                    size: s.size,
                    flip: (s.flip_h, s.flip_v),
                    line: s.kind.is_line(),
                    aspect: false,
                }
            }
        })
    }

    pub fn all(layout: &Layout) -> impl DoubleEndedIterator<Item = Object> {
        (0..layout.pictures.len())
            .map(Self::Picture)
            .chain((0..layout.charts.len()).map(Self::Chart))
            .chain((0..layout.shapes.len()).map(Self::Shape))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectDrag {
    pub object: Option<Object>,
    pub press: Pos2,
    pub from: (Pos2, Pos2),
    pub now: (Pos2, Pos2),
    pub handle: (u8, u8),
    pub aspect: bool,
}

impl ObjectDrag {
    pub fn new(object: Option<Object>, press: Pos2, from: (Pos2, Pos2), handle: (u8, u8)) -> Self {
        Self {
            object,
            press,
            from,
            now: from,
            handle,
            aspect: false,
        }
    }

    pub fn update(&mut self, pos: Pos2) {
        let delta = pos - self.press;
        let (mut a, mut b) = self.from;
        if self.handle == (0, 0) {
            a += delta;
            b += delta;
        } else {
            match self.handle.0 {
                1 => a.x += delta.x,
                2 => b.x += delta.x,
                _ => {}
            }
            match self.handle.1 {
                1 => a.y += delta.y,
                2 => b.y += delta.y,
                _ => {}
            }
            let (w0, h0) = (
                (self.from.1.x - self.from.0.x).abs(),
                (self.from.1.y - self.from.0.y).abs(),
            );
            if self.aspect && self.handle.0 != 0 && self.handle.1 != 0 && w0 > 0.0 {
                let height = (b.x - a.x).abs() * h0 / w0;
                if self.handle.1 == 1 {
                    a.y = b.y + height * (a.y - b.y).signum();
                } else {
                    b.y = a.y + height * (b.y - a.y).signum();
                }
            }
        }
        self.now = (a, b);
    }

    pub fn rect(&self) -> Rect {
        Rect::from_two_pos(self.now.0, self.now.1)
    }

    pub fn flip(&self) -> (bool, bool) {
        (self.now.1.x < self.now.0.x, self.now.1.y < self.now.0.y)
    }
}

pub fn ends(rect: Rect, flip: (bool, bool)) -> (Pos2, Pos2) {
    let (ax, bx) = if flip.0 {
        (rect.right(), rect.left())
    } else {
        (rect.left(), rect.right())
    };
    let (ay, by) = if flip.1 {
        (rect.bottom(), rect.top())
    } else {
        (rect.top(), rect.bottom())
    };
    (pos2(ax, ay), pos2(bx, by))
}

pub fn handles(rect: Rect, flip: (bool, bool), line: bool) -> Vec<(Pos2, (u8, u8))> {
    let (a, b) = ends(rect, flip);
    if line {
        return vec![(a, (1, 1)), (b, (2, 2))];
    }
    let low = |va: f32, vb: f32| if va <= vb { 1 } else { 2 };
    let high = |va: f32, vb: f32| if va <= vb { 2 } else { 1 };
    let xs = [
        (rect.left(), low(a.x, b.x)),
        (rect.center().x, 0),
        (rect.right(), high(a.x, b.x)),
    ];
    let ys = [
        (rect.top(), low(a.y, b.y)),
        (rect.center().y, 0),
        (rect.bottom(), high(a.y, b.y)),
    ];
    let mut out = Vec::new();
    for (x, hx) in xs {
        for (y, hy) in ys {
            if hx != 0 || hy != 0 {
                out.push((pos2(x, y), (hx, hy)));
            }
        }
    }
    out
}

pub fn near_segment(p: Pos2, a: Pos2, b: Pos2, tolerance: f32) -> bool {
    let ab = b - a;
    let length = ab.length_sq();
    let t = if length == 0.0 {
        0.0
    } else {
        ((p - a).dot(ab) / length).clamp(0.0, 1.0)
    };
    (a + ab * t).distance(p) <= tolerance
}

fn line_px(width: u16, zoom: f32) -> f32 {
    (fonts::px(f32::from(width) / 100.0) * zoom).max(1.0)
}

fn arrow_head(painter: &Painter, tip: Pos2, from: Pos2, stroke: Stroke, zoom: f32) {
    let direction = (tip - from).normalized();
    if !direction.x.is_finite() || !direction.y.is_finite() {
        return;
    }
    let size = (stroke.width * 3.0).max(theme::ARROW_HEAD * zoom);
    let base = tip - direction * size;
    let side = vec2(-direction.y, direction.x) * size / 2.0;
    painter.add(eframe::egui::Shape::convex_polygon(
        vec![tip, base + side, base - side],
        stroke.color,
        Stroke::NONE,
    ));
}

pub fn shape_points(shape: &Shape, rect: Rect, flip: (bool, bool)) -> Vec<(f64, f64)> {
    let (w, h) = (f64::from(rect.width()), f64::from(rect.height()));
    outline(shape.kind, w, h)
        .into_iter()
        .map(|(x, y)| {
            (
                if flip.0 { w - x } else { x },
                if flip.1 { h - y } else { y },
            )
        })
        .collect()
}

pub fn paint_shape(painter: &Painter, rect: Rect, shape: &Shape, flip: (bool, bool), zoom: f32) {
    let points = shape_points(shape, rect, flip);
    let screen: Vec<Pos2> = points
        .iter()
        .map(|(x, y)| rect.min + vec2(*x as f32, *y as f32))
        .collect();
    let stroke = shape
        .line
        .map(|c| Stroke::new(line_px(shape.line_width, zoom), render::color(c)));
    if shape.kind.is_line() {
        if let (Some(stroke), [a, b]) = (stroke, screen.as_slice()) {
            painter.line_segment([*a, *b], stroke);
            if matches!(shape.kind, ShapeKind::Arrow | ShapeKind::DoubleArrow) {
                arrow_head(painter, *b, *a, stroke, zoom);
            }
            if shape.kind == ShapeKind::DoubleArrow {
                arrow_head(painter, *a, *b, stroke, zoom);
            }
        }
        return;
    }
    if let Some(fill) = shape.fill.map(render::color) {
        let mut mesh = Mesh::default();
        for p in &screen {
            mesh.colored_vertex(*p, fill);
        }
        for [a, b, c] in triangulate(&points) {
            mesh.add_triangle(a as u32, b as u32, c as u32);
        }
        painter.add(mesh);
        if stroke.is_none() {
            painter.add(PathShape::closed_line(
                screen.clone(),
                Stroke::new(1.0, fill),
            ));
        }
    }
    if let Some(stroke) = stroke {
        painter.add(PathShape::closed_line(screen, stroke));
    }
    if !shape.text.is_empty() {
        let galley_painter = painter.with_clip_rect(painter.clip_rect().intersect(rect));
        paint_text(&galley_painter, rect, shape, zoom);
    }
}

pub fn text_font(shape: &Shape, zoom: f32) -> FontId {
    FontId::new(
        fonts::px(f32::from(shape.font_size) / 10.0) * zoom,
        fonts::family_for(DEFAULT_FONT, shape.bold, shape.italic),
    )
}

pub fn text_area(rect: Rect, zoom: f32) -> Rect {
    rect.shrink2(vec2(theme::SHAPE_INSET_X, theme::SHAPE_INSET_Y) * zoom)
}

pub fn text_align(shape: &Shape) -> Align {
    match shape.align {
        TextAlign::Left => Align::LEFT,
        TextAlign::Center => Align::Center,
        TextAlign::Right => Align::RIGHT,
    }
}

fn paint_text(painter: &Painter, rect: Rect, shape: &Shape, zoom: f32) {
    let area = text_area(rect, zoom);
    let color = render::color(shape.color);
    let mut job = LayoutJob::simple(
        shape.text.clone(),
        text_font(shape, zoom),
        color,
        area.width().max(1.0),
    );
    job.halign = text_align(shape);
    let galley = painter.layout_job(job);
    let x = match shape.align {
        TextAlign::Left => area.left(),
        TextAlign::Center => area.center().x,
        TextAlign::Right => area.right(),
    };
    let y = match shape.text_anchor {
        TextAnchor::Top => area.top(),
        TextAnchor::Middle => area.center().y - galley.size().y / 2.0,
        TextAnchor::Bottom => area.bottom() - galley.size().y,
    };
    painter.galley(pos2(x, y), galley, color);
}

pub fn paint_selection(painter: &Painter, rect: Rect, flip: (bool, bool), line: bool) {
    let stroke = Stroke::new(1.0, theme::chart_selected());
    if !line {
        painter.rect_stroke(rect, CornerRadius::ZERO, stroke, StrokeKind::Outside);
    }
    for (pos, _) in handles(rect, flip, line) {
        let handle = Rect::from_center_size(pos, vec2(theme::CHART_HANDLE, theme::CHART_HANDLE));
        painter.rect(
            handle,
            CornerRadius::ZERO,
            theme::cell_bg(),
            stroke,
            StrokeKind::Inside,
        );
    }
}

type TextureKey = (usize, usize);

fn texture_key(picture: &Picture) -> TextureKey {
    (picture.data.as_ptr() as usize, picture.data.len())
}

fn natural_size(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

fn decode(bytes: &[u8]) -> Option<ColorImage> {
    let mut image = image::load_from_memory(bytes).ok()?;
    if image.width() > MAX_TEXTURE_SIDE || image.height() > MAX_TEXTURE_SIDE {
        image = image.thumbnail(MAX_TEXTURE_SIDE, MAX_TEXTURE_SIDE);
    }
    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

pub type Textures = HashMap<TextureKey, Option<TextureHandle>>;

impl Spreadsheet {
    pub fn prepare_pictures(&mut self, ctx: &Context) {
        let live: HashSet<TextureKey> = self
            .doc
            .book()
            .sheets()
            .iter()
            .flat_map(|sheet| sheet.layout().pictures.iter().map(texture_key))
            .collect();
        self.textures.retain(|key, _| live.contains(key));
        let missing: Vec<(TextureKey, std::sync::Arc<[u8]>)> = self
            .sheet()
            .layout()
            .pictures
            .iter()
            .filter(|p| !self.textures.contains_key(&texture_key(p)))
            .map(|p| (texture_key(p), p.data.clone()))
            .collect();
        for (key, data) in missing {
            let texture = decode(&data)
                .map(|image| ctx.load_texture("picture", image, TextureOptions::LINEAR));
            self.textures.insert(key, texture);
        }
    }

    pub fn paint_picture(&self, painter: &Painter, rect: Rect, picture: &Picture) {
        match self
            .textures
            .get(&texture_key(picture))
            .and_then(Option::as_ref)
        {
            Some(texture) => {
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                painter.image(texture.id(), rect, uv, Color32::WHITE);
            }
            None => {
                painter.rect(
                    rect,
                    CornerRadius::ZERO,
                    theme::cell_bg(),
                    Stroke::new(1.0, theme::chart_selected()),
                    StrokeKind::Inside,
                );
            }
        }
    }

    pub fn chart_index(&self) -> Option<usize> {
        match self.object {
            Some(Object::Chart(index)) => Some(index),
            _ => None,
        }
    }

    pub fn selected_shape(&self) -> Option<(usize, &Shape)> {
        match self.object {
            Some(Object::Shape(index)) => self
                .sheet()
                .layout()
                .shapes
                .get(index)
                .map(|shape| (index, shape)),
            _ => None,
        }
    }

    pub fn selected_picture(&self) -> Option<(usize, &Picture)> {
        match self.object {
            Some(Object::Picture(index)) => self
                .sheet()
                .layout()
                .pictures
                .get(index)
                .map(|picture| (index, picture)),
            _ => None,
        }
    }

    pub fn insert_picture(&mut self, bytes: Vec<u8>) {
        let size = natural_size(&bytes);
        let picture = size.and_then(|(w, h)| {
            let limit = self.grid_area / self.zoom * theme::PICTURE_FIT;
            let scale = (limit.x / w as f32).min(limit.y / h as f32).min(1.0);
            let size = (
                ((w as f32 * scale).round() as u32).max(1),
                ((h as f32 * scale).round() as u32).max(1),
            );
            Picture::new(bytes, self.sel.active, size)
        });
        let Some(picture) = picture else {
            self.dialog = Some(Dialog::Error {
                title: text::picture_failed(),
                message: text::picture_unsupported().to_owned(),
            });
            return;
        };
        self.doc.set_layout(|layout| layout.pictures.push(picture));
        self.object = Some(Object::Picture(self.sheet().layout().pictures.len() - 1));
        self.shape_tool = None;
        self.tab = crate::ribbon::Tab::PictureFormat;
    }

    pub fn pick_picture(&mut self, ctx: &Context) {
        if let Some(bytes) = crate::files::pick_image(&self.inbox, ctx) {
            self.insert_picture(bytes);
        }
    }

    pub fn start_shape_tool(&mut self, kind: ShapeKind) {
        self.commit_shape_text();
        self.object = None;
        self.shape_tool = Some(kind);
    }

    pub fn add_shape(&mut self, mut shape: Shape) {
        self.shape_tool = None;
        let text_box = shape.kind == ShapeKind::TextBox;
        if !shape.kind.is_line() {
            shape.size = (shape.size.0.max(1), shape.size.1.max(1));
        }
        self.doc.set_layout(|layout| layout.shapes.push(shape));
        let index = self.sheet().layout().shapes.len() - 1;
        self.object = Some(Object::Shape(index));
        self.tab = crate::ribbon::Tab::ShapeFormat;
        if text_box {
            self.begin_shape_text(index);
        }
    }

    pub fn update_shape(&mut self, index: usize, f: impl FnOnce(&mut Shape)) {
        self.doc.set_layout(|layout| {
            if let Some(shape) = layout.shapes.get_mut(index) {
                f(shape);
            }
        });
    }

    pub fn update_picture(&mut self, index: usize, f: impl FnOnce(&mut Picture)) {
        self.doc.set_layout(|layout| {
            if let Some(picture) = layout.pictures.get_mut(index) {
                f(picture);
            }
        });
    }

    pub fn delete_object(&mut self) {
        self.shape_edit = None;
        let Some(object) = self.object.take() else {
            return;
        };
        self.doc.set_layout(|layout| match object {
            Object::Picture(i) if i < layout.pictures.len() => {
                layout.pictures.remove(i);
            }
            Object::Chart(i) if i < layout.charts.len() => {
                layout.charts.remove(i);
            }
            Object::Shape(i) if i < layout.shapes.len() => {
                layout.shapes.remove(i);
            }
            _ => {}
        });
    }

    pub fn reorder_object(&mut self, forward: bool) {
        self.commit_shape_text();
        fn step<T>(items: &mut [T], index: usize, forward: bool) -> Option<usize> {
            let target = if forward {
                index.checked_add(1).filter(|t| *t < items.len())?
            } else {
                index.checked_sub(1)?
            };
            items.swap(index, target);
            Some(target)
        }
        let Some(object) = self.object else {
            return;
        };
        let mut moved = None;
        self.doc.set_layout(|layout| {
            moved = match object {
                Object::Picture(i) => step(&mut layout.pictures, i, forward).map(Object::Picture),
                Object::Chart(i) => step(&mut layout.charts, i, forward).map(Object::Chart),
                Object::Shape(i) => step(&mut layout.shapes, i, forward).map(Object::Shape),
            };
        });
        if moved.is_some() {
            self.object = moved;
        }
    }

    pub fn reset_picture(&mut self, index: usize) {
        let Some(picture) = self.sheet().layout().pictures.get(index) else {
            return;
        };
        if let Some(size) = natural_size(&picture.data) {
            self.update_picture(index, |p| p.size = size);
        }
    }

    pub fn begin_shape_text(&mut self, index: usize) {
        let text = match self.sheet().layout().shapes.get(index) {
            Some(shape) if !shape.kind.is_line() => shape.text.clone(),
            _ => return,
        };
        self.object = Some(Object::Shape(index));
        self.shape_edit = Some(ShapeEdit {
            index,
            text,
            focus: true,
        });
    }

    pub fn commit_shape_text(&mut self) {
        let Some(edit) = self.shape_edit.take() else {
            return;
        };
        let changed = self
            .sheet()
            .layout()
            .shapes
            .get(edit.index)
            .is_some_and(|shape| shape.text != edit.text);
        if changed {
            self.update_shape(edit.index, |shape| shape.text = edit.text);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShapeEdit {
    pub index: usize,
    pub text: String,
    pub focus: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragging_a_handle_past_the_opposite_edge_flips() {
        let rect = Rect::from_min_max(pos2(10.0, 10.0), pos2(50.0, 30.0));
        let (_, right) = handles(rect, (false, false), false)
            .into_iter()
            .find(|(p, _)| *p == pos2(50.0, 20.0))
            .unwrap();
        let mut drag = ObjectDrag::new(None, pos2(50.0, 20.0), ends(rect, (false, false)), right);
        drag.update(pos2(0.0, 20.0));
        assert_eq!(
            drag.rect(),
            Rect::from_min_max(pos2(0.0, 10.0), pos2(10.0, 30.0))
        );
        assert_eq!(drag.flip(), (true, false));
    }

    #[test]
    fn corner_drag_keeps_picture_proportions() {
        let rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(40.0, 20.0));
        let mut drag = ObjectDrag::new(None, pos2(40.0, 20.0), ends(rect, (false, false)), (2, 2));
        drag.aspect = true;
        drag.update(pos2(80.0, 25.0));
        assert_eq!(drag.rect().size(), vec2(80.0, 40.0));
    }

    #[test]
    fn line_handles_follow_flips() {
        let rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 10.0));
        let handles = handles(rect, (false, true), true);
        assert_eq!(handles[0].0, pos2(0.0, 10.0));
        assert_eq!(handles[1].0, pos2(10.0, 0.0));
        assert!(near_segment(
            pos2(5.0, 5.0),
            handles[0].0,
            handles[1].0,
            1.0
        ));
        assert!(!near_segment(
            pos2(0.0, 0.0),
            handles[0].0,
            handles[1].0,
            1.0
        ));
    }
}
