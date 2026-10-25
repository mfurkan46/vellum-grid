mod actions;
mod data;
mod formulas;
mod help;
mod home;
mod insert;
mod page_layout;
mod review;
mod view;
pub mod widgets;

use eframe::egui::{self, CornerRadius, Frame, Id, Margin, Rect, Sense, Shadow, Stroke, Ui, vec2};

use crate::app::Spreadsheet;
use crate::icons::{self, Icon};
use crate::{fluent, text, theme};
use vellum_ui::motion;
use vellum_ui::widgets::tab_button;

pub use widgets::chrome_text;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tab {
    #[default]
    Home,
    Insert,
    PageLayout,
    Formulas,
    Data,
    Review,
    View,
    Help,
    TableDesign,
    ChartDesign,
    Sparkline,
    ShapeFormat,
    PictureFormat,
}

impl Tab {
    const ALL: [Self; 8] = [
        Self::Home,
        Self::Insert,
        Self::PageLayout,
        Self::Formulas,
        Self::Data,
        Self::Review,
        Self::View,
        Self::Help,
    ];
    const CONTEXTUAL: [Self; 5] = [
        Self::TableDesign,
        Self::ChartDesign,
        Self::Sparkline,
        Self::ShapeFormat,
        Self::PictureFormat,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Home => text::home(),
            Self::Insert => text::insert(),
            Self::PageLayout => text::page_layout(),
            Self::Formulas => text::formulas(),
            Self::Data => text::data(),
            Self::Review => text::review(),
            Self::View => text::view(),
            Self::Help => text::help(),
            Self::TableDesign => text::table_design(),
            Self::ChartDesign => text::chart_design(),
            Self::Sparkline => text::sparkline_tab(),
            Self::ShapeFormat => text::shape_format(),
            Self::PictureFormat => text::picture_format(),
        }
    }
}

impl Spreadsheet {
    pub fn ribbon_tabs(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            if tab_button(ui, text::file(), false).clicked() {
                self.backstage = Some(crate::backstage::Page::Home);
            }
            let table = self.active_table().map(|_| Tab::TableDesign);
            let chart = self.selected_chart().map(|_| Tab::ChartDesign);
            let spark = self.sparkline_at(self.sel.active).map(|_| Tab::Sparkline);
            let shape = self.selected_shape().map(|_| Tab::ShapeFormat);
            let picture = self.selected_picture().map(|_| Tab::PictureFormat);
            if (table.is_none() && self.tab == Tab::TableDesign)
                || (chart.is_none() && self.tab == Tab::ChartDesign)
                || (spark.is_none() && self.tab == Tab::Sparkline)
                || (shape.is_none() && self.tab == Tab::ShapeFormat)
                || (picture.is_none() && self.tab == Tab::PictureFormat)
            {
                self.tab = Tab::Home;
            }
            let contextual = [table, chart, spark, shape, picture];
            let mut selected = None;
            for tab in Tab::ALL.into_iter().chain(contextual.into_iter().flatten()) {
                let current = self.tab == tab && !self.ribbon_collapsed;
                let response = tab_button(ui, tab.label(), current);
                if current {
                    selected = Some(response.rect);
                }
                if response.clicked() {
                    self.tab = tab;
                    self.ribbon_collapsed = false;
                }
            }
            if let Some(rect) = selected {
                motion::tab_marker(ui, Id::unique("ribbon-tab-marker"), rect, theme::accent());
            }
        });
    }

    pub fn ribbon(&mut self, ui: &mut Ui) {
        let card = Frame::NONE
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
        let response = card.show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            let ctx = ui.ctx().clone();
            let mut shown = 1.0;
            for tab in Tab::ALL.into_iter().chain(Tab::CONTEXTUAL) {
                let t = motion::reveal(
                    &ctx,
                    Id::unique(("ribbon-tab", tab)),
                    tab == self.tab,
                    theme::RIBBON_SWITCH_ANIMATION,
                );
                if tab == self.tab {
                    shown = t;
                }
            }
            let slide = vec2(0.0, theme::RIBBON_SWITCH_SLIDE);
            egui::ScrollArea::horizontal()
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    motion::revealed(ui, shown, slide, |ui| self.ribbon_groups(ui));
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
        if collapse
            .on_hover_text(format!("{} (Ctrl+F1)", text::collapse_ribbon()))
            .clicked()
        {
            self.ribbon_collapsed = true;
        }
    }

    fn ribbon_groups(&mut self, ui: &mut Ui) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, theme::RIBBON_ROW_GAP);
            match self.tab {
                Tab::Home => self.home(ui),
                Tab::Insert => self.insert_tab(ui),
                Tab::PageLayout => self.page_layout(ui),
                Tab::Formulas => self.formulas(ui),
                Tab::Data => self.data_tab(ui),
                Tab::Review => self.review(ui),
                Tab::View => self.view_tab(ui),
                Tab::Help => self.help(ui),
                Tab::TableDesign => self.design_tab(ui),
                Tab::ChartDesign => self.chart_design_tab(ui),
                Tab::Sparkline => self.sparkline_tab(ui),
                Tab::ShapeFormat => self.shape_format_tab(ui),
                Tab::PictureFormat => self.picture_format_tab(ui),
            }
        });
    }
}
