use eframe::egui::{Popup, Ui};
use vellum_grid_core::{Chart, ChartKind, Grouping, Legend};

use crate::app::Spreadsheet;
use crate::dialogs::Dialog;
use crate::icons::Icon;
use crate::ribbon::widgets::{big, group};
use crate::{fluent, menu, text, theme};

pub struct ChartType {
    pub kind: ChartKind,
    pub grouping: Grouping,
    pub markers: bool,
    pub label: text::Label,
}

const fn kind(kind: ChartKind, grouping: Grouping, markers: bool, label: text::Label) -> ChartType {
    ChartType {
        kind,
        grouping,
        markers,
        label,
    }
}

pub const COLUMN_TYPES: [ChartType; 3] = [
    kind(
        ChartKind::Column,
        Grouping::Clustered,
        false,
        text::clustered_column,
    ),
    kind(
        ChartKind::Column,
        Grouping::Stacked,
        false,
        text::stacked_column,
    ),
    kind(
        ChartKind::Column,
        Grouping::Percent,
        false,
        text::percent_column,
    ),
];
pub const BAR_TYPES: [ChartType; 3] = [
    kind(
        ChartKind::Bar,
        Grouping::Clustered,
        false,
        text::clustered_bar,
    ),
    kind(ChartKind::Bar, Grouping::Stacked, false, text::stacked_bar),
    kind(ChartKind::Bar, Grouping::Percent, false, text::percent_bar),
];
pub const LINE_TYPES: [ChartType; 4] = [
    kind(
        ChartKind::Line,
        Grouping::Clustered,
        false,
        text::line_plain,
    ),
    kind(
        ChartKind::Line,
        Grouping::Stacked,
        false,
        text::stacked_line,
    ),
    kind(
        ChartKind::Line,
        Grouping::Percent,
        false,
        text::percent_line,
    ),
    kind(
        ChartKind::Line,
        Grouping::Clustered,
        true,
        text::line_markers,
    ),
];
pub const AREA_TYPES: [ChartType; 3] = [
    kind(
        ChartKind::Area,
        Grouping::Clustered,
        false,
        text::area_plain,
    ),
    kind(
        ChartKind::Area,
        Grouping::Stacked,
        false,
        text::stacked_area,
    ),
    kind(
        ChartKind::Area,
        Grouping::Percent,
        false,
        text::percent_area,
    ),
];
pub const PIE_TYPES: [ChartType; 2] = [
    kind(ChartKind::Pie, Grouping::Clustered, false, text::chart_pie),
    kind(
        ChartKind::Doughnut,
        Grouping::Clustered,
        false,
        text::doughnut,
    ),
];
pub const SCATTER_TYPES: [ChartType; 1] = [kind(
    ChartKind::Scatter,
    Grouping::Clustered,
    false,
    text::chart_scatter,
)];

pub fn families() -> [(Icon, &'static str, &'static str, &'static [ChartType]); 6] {
    [
        (
            Icon::ChartColumn,
            text::chart_column(),
            text::chart_column_tip(),
            &COLUMN_TYPES,
        ),
        (
            Icon::ChartBar,
            text::chart_bar(),
            text::chart_bar_tip(),
            &BAR_TYPES,
        ),
        (
            Icon::ChartLine,
            text::chart_line(),
            text::chart_line_tip(),
            &LINE_TYPES,
        ),
        (
            Icon::Tinted(fluent::DATA_HISTOGRAM, theme::tint_blue()),
            text::chart_area(),
            text::chart_area_tip(),
            &AREA_TYPES,
        ),
        (
            Icon::ChartPie,
            text::chart_pie(),
            text::chart_pie_tip(),
            &PIE_TYPES,
        ),
        (
            Icon::ChartScatter,
            text::chart_scatter(),
            text::chart_scatter_tip(),
            &SCATTER_TYPES,
        ),
    ]
}

impl Spreadsheet {
    pub fn selected_chart(&self) -> Option<&Chart> {
        self.chart_index()
            .and_then(|index| self.sheet().layout().charts.get(index))
    }

    pub fn apply_chart_type(&mut self, chart_type: &ChartType) {
        let (kind, grouping, markers) = (chart_type.kind, chart_type.grouping, chart_type.markers);
        if self.chart_index().is_none() {
            self.insert_chart(kind);
        }
        self.update_chart(|chart| {
            chart.kind = kind;
            chart.grouping = grouping;
            chart.markers = markers;
        });
    }

    pub fn update_chart(&mut self, f: impl FnOnce(&mut Chart)) {
        let Some(index) = self.chart_index() else {
            return;
        };
        self.doc.set_layout(|layout| {
            if let Some(chart) = layout.charts.get_mut(index) {
                f(chart);
            }
        });
    }

    pub fn chart_type_menu(&mut self, ui: &mut Ui, types: &[ChartType]) {
        let current = self
            .selected_chart()
            .map(|c| (c.kind, c.grouping, c.markers));
        for chart_type in types {
            let mut on =
                current == Some((chart_type.kind, chart_type.grouping, chart_type.markers));
            let was = on;
            menu::check(ui, &mut on, (chart_type.label)());
            if on != was {
                self.apply_chart_type(chart_type);
                ui.close();
            }
        }
    }

    pub fn chart_groups(&mut self, ui: &mut Ui) {
        group(ui, text::charts(), |ui| {
            ui.horizontal_top(|ui| {
                for (icon, label, hint, types) in families() {
                    let current = self
                        .selected_chart()
                        .is_some_and(|chart| types.iter().any(|t| t.kind == chart.kind));
                    let button =
                        crate::ribbon::widgets::big_checked(ui, icon, label, hint, current);
                    Popup::menu(&button)
                        .width(theme::MENU_WIDTH)
                        .show(|ui| self.chart_type_menu(ui, types));
                }
            });
        });
    }

    pub fn chart_design_tab(&mut self, ui: &mut Ui) {
        let Some(chart) = self.selected_chart().cloned() else {
            return;
        };
        group(ui, text::chart_layouts(), |ui| {
            let icon = Icon::Tinted(fluent::DATA_BAR_VERTICAL, theme::tint_blue());
            let button = big(
                ui,
                icon,
                text::add_chart_element(),
                text::add_chart_element(),
                false,
            );
            Popup::menu(&button)
                .width(theme::MENU_WIDTH)
                .show(|ui| self.chart_element_menu(ui, &chart));
        });
        group(ui, text::chart_type(), |ui| {
            let icon = Icon::ChartColumn;
            let button = big(
                ui,
                icon,
                text::change_chart_type(),
                text::change_chart_type(),
                false,
            );
            Popup::menu(&button).width(theme::MENU_WIDTH).show(|ui| {
                for (_, label, _, types) in families() {
                    menu::sub(ui, None, label, |ui| self.chart_type_menu(ui, types));
                }
            });
        });
    }

    fn chart_element_menu(&mut self, ui: &mut Ui, chart: &Chart) {
        let round = chart.kind.is_round();
        if !round {
            menu::sub(ui, None, text::axis_titles(), |ui| {
                for (x, label, current) in [
                    (true, text::primary_horizontal(), &chart.x_title),
                    (false, text::primary_vertical(), &chart.y_title),
                ] {
                    let mut on = current.is_some();
                    let was = on;
                    menu::check(ui, &mut on, label);
                    if on != was {
                        if on {
                            self.open_axis_title(x);
                        } else {
                            self.update_chart(|c| {
                                if x {
                                    c.x_title = None;
                                } else {
                                    c.y_title = None;
                                }
                            });
                        }
                        ui.close();
                    }
                }
            });
        }
        if menu::item(ui, text::edit_chart_title())
            && let Some(index) = self.chart_index()
        {
            self.open_chart_title(index);
        }
        let mut labels = chart.labels;
        menu::check(ui, &mut labels, text::data_labels());
        if labels != chart.labels {
            self.update_chart(|c| c.labels = labels);
        }
        if !round {
            let mut gridlines = chart.gridlines;
            menu::check(ui, &mut gridlines, text::gridlines_item());
            if gridlines != chart.gridlines {
                self.update_chart(|c| c.gridlines = gridlines);
            }
        }
        if chart.kind == ChartKind::Line {
            let mut markers = chart.markers;
            menu::check(ui, &mut markers, text::markers());
            if markers != chart.markers {
                self.update_chart(|c| c.markers = markers);
            }
        }
        menu::sub(ui, None, text::legend(), |ui| {
            for (legend, label) in [
                (Legend::Hidden, text::none()),
                (Legend::Right, text::legend_right()),
                (Legend::Top, text::legend_top()),
                (Legend::Left, text::legend_left()),
                (Legend::Bottom, text::legend_bottom()),
            ] {
                let mut on = chart.legend == legend;
                let was = on;
                menu::check(ui, &mut on, label);
                if on && !was {
                    self.update_chart(|c| c.legend = legend);
                    ui.close();
                }
            }
        });
    }

    fn open_axis_title(&mut self, x: bool) {
        let Some(index) = self.chart_index() else {
            return;
        };
        self.dialog = Some(Dialog::ChartAxisTitle {
            index,
            x,
            title: text::axis_title().to_owned(),
        });
    }
}
