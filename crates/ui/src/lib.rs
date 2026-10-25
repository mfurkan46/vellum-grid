use std::sync::atomic::{AtomicBool, Ordering};

pub mod backstage;
pub mod dialog;
pub mod fluent;
pub mod fonts;
pub mod icons;
pub mod logo;
pub mod menu;
pub mod motion;
pub mod options;
pub mod palette;
pub mod settings;
pub mod text;
pub mod theme;
pub mod widgets;
pub mod window;

pub use eframe::egui::Color32;

pub fn rgb(value: u32) -> Color32 {
    Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

static DARK: AtomicBool = AtomicBool::new(false);
static DARK_CANVAS: AtomicBool = AtomicBool::new(false);
static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn dark() -> bool {
    DARK.load(Ordering::Relaxed)
}

pub fn set_dark(on: bool) {
    DARK.store(on, Ordering::Relaxed);
}

pub fn dark_canvas() -> bool {
    DARK_CANVAS.load(Ordering::Relaxed)
}

pub fn set_dark_canvas(on: bool) {
    DARK_CANVAS.store(on, Ordering::Relaxed);
}

pub fn english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

pub fn set_english(on: bool) {
    ENGLISH.store(on, Ordering::Relaxed);
}

#[macro_export]
macro_rules! texts {
    ($($(#[$meta:meta])* $name:ident: $tr:literal, $en:literal;)*) => {
        $(
            $(#[$meta])*
            #[inline]
            pub fn $name() -> &'static str {
                if $crate::english() { $en } else { $tr }
            }
        )*
    };
}

#[macro_export]
macro_rules! colors {
    ($($(#[$meta:meta])* $name:ident: $light:expr, $dark:expr;)*) => {
        $(
            $(#[$meta])*
            #[inline]
            pub fn $name() -> $crate::Color32 {
                if $crate::dark() { $dark } else { $light }
            }
        )*
    };
}

#[macro_export]
macro_rules! canvas_colors {
    ($($(#[$meta:meta])* $name:ident: $light:expr, $dark:expr;)*) => {
        $(
            $(#[$meta])*
            #[inline]
            pub fn $name() -> $crate::Color32 {
                if $crate::dark_canvas() { $dark } else { $light }
            }
        )*
    };
}

#[cfg(test)]
mod tests {
    crate::texts! {
        sample: "Kaydet", "Save";
    }

    crate::colors! {
        canvas: crate::Color32::WHITE, crate::Color32::BLACK;
    }

    #[test]
    fn texts_and_colors_follow_the_switches() {
        assert_eq!(sample(), "Kaydet");
        assert_eq!(canvas(), crate::Color32::WHITE);
        crate::set_english(true);
        crate::set_dark(true);
        assert_eq!(sample(), "Save");
        assert_eq!(canvas(), crate::Color32::BLACK);
        crate::set_english(false);
        crate::set_dark(false);
        assert_eq!(
            crate::rgb(0x107C41),
            crate::Color32::from_rgb(0x10, 0x7C, 0x41)
        );
    }
}
