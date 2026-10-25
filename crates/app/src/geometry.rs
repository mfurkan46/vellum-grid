use std::collections::BTreeMap;

use vellum_grid_core::{DEFAULT_COLUMN_WIDTH, DEFAULT_ROW_HEIGHT, Layout, MAX_COLS, MAX_ROWS};

use crate::fonts::px;

#[derive(Debug, Clone)]
pub struct Axis {
    default: f32,
    custom: BTreeMap<u32, f32>,
    max: u32,
}

impl Axis {
    pub fn columns(layout: &Layout, digit: f32) -> Self {
        let width = |units: f64| (units as f32 * digit).round();
        let mut custom: BTreeMap<u32, f32> = layout
            .col_widths
            .iter()
            .map(|(col, units)| (*col, width(*units)))
            .collect();
        custom.extend(layout.hidden_cols.iter().map(|col| (*col, 0.0)));
        Self {
            default: width(DEFAULT_COLUMN_WIDTH),
            custom,
            max: MAX_COLS,
        }
    }

    pub fn rows(layout: &Layout, zoom: f32) -> Self {
        let height = |points: f64| (px(points as f32) * zoom).round();
        let mut custom: BTreeMap<u32, f32> = layout
            .row_heights
            .iter()
            .map(|(row, points)| (*row, height(*points)))
            .collect();
        custom.extend(layout.hidden_rows.iter().map(|row| (*row, 0.0)));
        Self {
            default: height(DEFAULT_ROW_HEIGHT),
            custom,
            max: MAX_ROWS,
        }
    }

    pub fn set(&mut self, index: u32, size: f32) {
        self.custom.insert(index, size.max(0.0));
    }

    pub fn size(&self, index: u32) -> f32 {
        self.custom.get(&index).copied().unwrap_or(self.default)
    }

    pub fn offset(&self, from: u32, to: u32) -> f32 {
        let (low, high, sign) = if to >= from {
            (from, to, 1.0)
        } else {
            (to, from, -1.0)
        };
        let base = self.default * (high - low) as f32;
        let extra: f32 = self
            .custom
            .range(low..high)
            .map(|(_, size)| size - self.default)
            .sum();
        sign * (base + extra)
    }

    pub fn index_at(&self, from: u32, distance: f32) -> u32 {
        let mut index = from;
        if distance >= 0.0 {
            let mut remaining = distance;
            while index + 1 < self.max {
                let size = self.size(index);
                if remaining < size {
                    break;
                }
                remaining -= size;
                index += 1;
            }
        } else {
            let mut remaining = -distance;
            while index > 0 && remaining > 0.0 {
                index -= 1;
                remaining -= self.size(index);
            }
        }
        index
    }

    pub fn fit(&self, from: u32, length: f32) -> (u32, u32) {
        let mut used = 0.0;
        let mut index = from;
        while index < self.max && used < length {
            used += self.size(index);
            index += 1;
        }
        let shown = index - from;
        let full = if used > length { shown - 1 } else { shown };
        (full.max(1), shown.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis() -> Axis {
        let mut layout = Layout::default();
        layout.col_widths.insert(1, 20.0);
        layout.col_widths.insert(3, 0.0);
        Axis::columns(&layout, 7.0)
    }

    #[test]
    fn default_width_is_standard() {
        let axis = Axis::columns(&Layout::default(), 7.0);
        assert_eq!(axis.size(0), 64.0);
        assert_eq!(Axis::rows(&Layout::default(), 1.0).size(5), 20.0);
        assert_eq!(Axis::rows(&Layout::default(), 2.0).size(5), 40.0);
    }

    #[test]
    fn offsets_include_custom_sizes() {
        let axis = axis();
        assert_eq!(axis.size(1), 140.0);
        assert_eq!(axis.offset(0, 2), 204.0);
        assert_eq!(axis.offset(0, 4), 268.0);
        assert_eq!(axis.offset(4, 0), -268.0);
        assert_eq!(axis.offset(5, 5), 0.0);
    }

    #[test]
    fn finds_index_at_position() {
        let axis = axis();
        assert_eq!(axis.index_at(0, 10.0), 0);
        assert_eq!(axis.index_at(0, 64.0), 1);
        assert_eq!(axis.index_at(0, 203.0), 1);
        assert_eq!(axis.index_at(0, 204.0), 2);
        assert_eq!(axis.index_at(0, 268.0), 4);
        assert_eq!(axis.index_at(5, -1.0), 4);
        assert_eq!(axis.index_at(5, -70.0), 2);
        assert_eq!(axis.index_at(0, -50.0), 0);
    }

    #[test]
    fn hidden_rows_take_no_space() {
        let mut layout = Layout::default();
        layout.hidden_rows.extend([1, 2]);
        let rows = Axis::rows(&layout, 1.0);
        assert_eq!(rows.size(1), 0.0);
        assert_eq!(rows.offset(0, 4), 40.0);
        assert_eq!(rows.index_at(0, 25.0), 3);
    }

    #[test]
    fn counts_fitting_items() {
        let axis = axis();
        assert_eq!(axis.fit(0, 300.0), (4, 5));
        assert_eq!(axis.fit(0, 268.0), (3, 3));
        assert_eq!(axis.fit(0, 10.0), (1, 1));
    }
}
