const THEME: [(u32, [f64; 5]); 10] = [
    (0xFFFFFF, [-0.05, -0.15, -0.25, -0.35, -0.5]),
    (0x000000, [0.5, 0.35, 0.25, 0.15, 0.05]),
    (0xE7E6E6, [-0.1, -0.25, -0.5, -0.75, -0.9]),
    (0x44546A, [0.8, 0.6, 0.4, -0.25, -0.5]),
    (0x4472C4, [0.8, 0.6, 0.4, -0.25, -0.5]),
    (0xED7D31, [0.8, 0.6, 0.4, -0.25, -0.5]),
    (0xA5A5A5, [0.8, 0.6, 0.4, -0.25, -0.5]),
    (0xFFC000, [0.8, 0.6, 0.4, -0.25, -0.5]),
    (0x5B9BD5, [0.8, 0.6, 0.4, -0.25, -0.5]),
    (0x70AD47, [0.8, 0.6, 0.4, -0.25, -0.5]),
];

pub const STANDARD: [u32; 10] = [
    0xC00000, 0xFF0000, 0xFFC000, 0xFFFF00, 0x92D050, 0x00B050, 0x00B0F0, 0x0070C0, 0x002060,
    0x7030A0,
];

pub fn tinted(rgb: u32, tint: f64) -> u32 {
    let channel = |shift: u32| {
        let c = f64::from((rgb >> shift) & 0xFF);
        let value = if tint < 0.0 {
            c * (1.0 + tint)
        } else {
            c + (255.0 - c) * tint
        };
        (value.round().clamp(0.0, 255.0) as u32) << shift
    };
    channel(16) | channel(8) | channel(0)
}

pub fn theme_rows() -> Vec<Vec<u32>> {
    let mut rows = vec![THEME.iter().map(|(rgb, _)| *rgb).collect()];
    for shade in 0..5 {
        rows.push(
            THEME
                .iter()
                .map(|(rgb, tints)| tinted(*rgb, tints[shade]))
                .collect(),
        );
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tints_lighten_and_darken() {
        assert_eq!(tinted(0x000000, 0.5), 0x808080);
        assert_eq!(tinted(0xFFFFFF, -0.5), 0x808080);
        assert_eq!(theme_rows().len(), 6);
    }
}
