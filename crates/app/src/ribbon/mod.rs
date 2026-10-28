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

use eframe::egui::Ui;

use crate::app::Spreadsheet;
use crate::text;
use vellum_ui::ribbon::{self, TabClick};

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
        let tabs: Vec<_> = Tab::ALL
            .into_iter()
            .chain(contextual.into_iter().flatten())
            .map(|tab| (tab, tab.label()))
            .collect();
        match ribbon::tabs(ui, &tabs, self.tab, self.ribbon_collapsed) {
            Some(TabClick::File) => self.backstage = Some(crate::backstage::Page::Home),
            Some(TabClick::Tab(tab)) => {
                self.tab = tab;
                self.ribbon_collapsed = false;
            }
            None => {}
        }
    }

    pub fn ribbon(&mut self, ui: &mut Ui) {
        let all: Vec<_> = Tab::ALL.into_iter().chain(Tab::CONTEXTUAL).collect();
        if ribbon::card(ui, &all, self.tab, |ui| self.ribbon_groups(ui)) {
            self.ribbon_collapsed = true;
        }
    }

    fn ribbon_groups(&mut self, ui: &mut Ui) {
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
    }
}
