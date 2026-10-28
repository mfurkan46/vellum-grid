use std::fmt::Debug;
use std::hash::Hash;

use eframe::egui::{
    self, CornerRadius, Frame, Id, Margin, Rect, Sense, Shadow, Stroke, Ui, scroll_area, vec2,
};

use crate::icons::{self, Icon};
use crate::widgets::tab_button;
use crate::{fluent, motion, text, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabClick<K> {
    File,
    Tab(K),
}

/// The ribbon tab row: "File" first, then `tabs`. The current tab is underlined unless the
/// ribbon is collapsed.
pub fn tabs<K: Copy + PartialEq>(
    ui: &mut Ui,
    tabs: &[(K, &str)],
    current: K,
    collapsed: bool,
) -> Option<TabClick<K>> {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let mut clicked = None;
        if tab_button(ui, text::file(), false).clicked() {
            clicked = Some(TabClick::File);
        }
        let mut selected = None;
        for &(tab, label) in tabs {
            let on = tab == current && !collapsed;
            let response = tab_button(ui, label, on);
            if on {
                selected = Some(response.rect);
            }
            if response.clicked() {
                clicked = Some(TabClick::Tab(tab));
            }
        }
        if let Some(rect) = selected {
            motion::tab_marker(ui, Id::unique("ribbon-tab-marker"), rect, theme::accent());
        }
        clicked
    })
    .inner
}

/// The ribbon card showing the groups of `current`; `all` lists every tab so switching
/// animates. Returns true when the collapse button is clicked.
pub fn card<K: Hash + Debug + Copy + Eq + Send + Sync + 'static>(
    ui: &mut Ui,
    all: &[K],
    current: K,
    add: impl FnOnce(&mut Ui),
) -> bool {
    let frame = Frame::NONE
        .fill(theme::ribbon_bg())
        .corner_radius(CornerRadius::same(theme::RIBBON_RADIUS))
        .stroke(Stroke::new(1.0, theme::ribbon_line()))
        .shadow(Shadow {
            offset: [0, 1],
            blur: theme::RIBBON_SHADOW_BLUR,
            spread: 0,
            color: theme::ribbon_shadow(),
        })
        .inner_margin(Margin::symmetric(
            theme::RIBBON_PADDING_X,
            theme::RIBBON_PADDING_Y,
        ));
    let response = frame.show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        let ctx = ui.ctx().clone();
        let mut shown = 1.0;
        for &tab in all {
            let t = motion::reveal(
                &ctx,
                Id::unique(("ribbon-tab", tab)),
                tab == current,
                theme::RIBBON_SWITCH_ANIMATION,
            );
            if tab == current {
                shown = t;
            }
        }
        let slide = vec2(0.0, theme::RIBBON_SWITCH_SLIDE);
        egui::ScrollArea::horizontal()
            .scroll_bar_visibility(scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                motion::revealed(ui, shown, slide, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing = vec2(2.0, theme::RIBBON_ROW_GAP);
                        add(ui);
                    });
                });
            });
    });
    let corner = response.response.rect.right_bottom();
    let area = Rect::from_min_size(
        corner - vec2(theme::COLLAPSE_SIZE + 6.0, theme::COLLAPSE_SIZE + 4.0),
        vec2(theme::COLLAPSE_SIZE, theme::COLLAPSE_SIZE),
    );
    let collapse = ui.interact(area, ui.scope_id().with("collapse-ribbon"), Sense::click());
    let hover = motion::hover_color(ui, &collapse, collapse.hovered(), theme::tool_hover());
    ui.painter().rect_filled(area, CornerRadius::same(3), hover);
    icons::paint(
        ui.painter(),
        area.shrink(3.0),
        Icon::Plain(fluent::CHEVRON_UP),
        theme::icon(),
        None,
    );
    collapse
        .on_hover_text(format!("{} (Ctrl+F1)", text::collapse_ribbon()))
        .clicked()
}
