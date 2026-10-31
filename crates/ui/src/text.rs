pub type Label = fn() -> &'static str;

crate::texts! {
    previous_page: "Önceki Sayfa", "Previous Page";
    next_page: "Sonraki Sayfa", "Next Page";
    coming_soon: "Bu özellik sonraki bir sürümde gelecek.", "This feature is coming in a later version.";
    no_fill: "Dolgu Yok", "No Fill";
    standard_colors: "Standart Renkler", "Standard Colors";
    theme_colors: "Tema Renkleri", "Theme Colors";
    just_now: "az önce", "just now";
    minute_ago: "dakika önce", "minute ago";
    minutes_ago: "dakika önce", "minutes ago";
    hour_ago: "saat önce", "hour ago";
    hours_ago: "saat önce", "hours ago";
    day_ago: "gün önce", "day ago";
    days_ago: "gün önce", "days ago";
    options: "Seçenekler", "Options";
    close: "Kapat", "Close";
    version: "Sürüm", "Version";
    minimize: "Simge Durumuna Küçült", "Minimize";
    maximize: "Ekranı Kapla", "Maximize";
    restore: "Önceki Boyut", "Restore Down";
    theme_label: "Tema:", "Theme:";
    language_label: "Arayüz dili:", "Display language:";
    theme_system: "Sistem ayarını kullan", "Use system setting";
    theme_light: "Açık", "Light";
    theme_dark: "Koyu", "Dark";
    canvas_label: "Belge zemini:", "Document background:";
    canvas_white: "Beyaz", "White";
    options_note: "Bu ayarlar bütün Vellum uygulamalarında geçerlidir.", "These settings apply to all Vellum apps.";
    file: "Dosya", "File";
    back: "Geri", "Back";
    collapse_ribbon: "Şeridi Daralt", "Collapse the Ribbon";
    zoom_level: "Yakınlaştırma düzeyi", "Zoom level";
    zoom_in: "Yakınlaştır", "Zoom In";
    zoom_out: "Uzaklaştır", "Zoom Out";
}

pub fn percent_of(value: impl std::fmt::Display) -> String {
    if crate::english() {
        format!("{value}%")
    } else {
        format!("%{value}")
    }
}
