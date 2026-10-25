use std::collections::HashMap;
use std::io::Cursor;

use calamine::{CellErrorType, Data, Reader, open_workbook_auto_from_rs};
use rust_xlsxwriter::{
    Chart as XlsxChart, Color as XlsxColor, ConditionalFormat2ColorScale,
    ConditionalFormat3ColorScale, ConditionalFormatAverage, ConditionalFormatAverageRule,
    ConditionalFormatBlank, ConditionalFormatCell, ConditionalFormatCellRule,
    ConditionalFormatCustomIcon, ConditionalFormatDataBar, ConditionalFormatDate,
    ConditionalFormatDateRule, ConditionalFormatDuplicate, ConditionalFormatError,
    ConditionalFormatFormula, ConditionalFormatIconSet, ConditionalFormatIconType,
    ConditionalFormatText, ConditionalFormatTextRule, ConditionalFormatTop,
    ConditionalFormatTopRule, ConditionalFormatType, DataValidation, DataValidationErrorStyle,
    DataValidationRule, FilterCondition, FilterCriteria, Format as XlsxFormat, FormatAlign,
    FormatBorder, FormatDiagonalBorder, FormatPattern, FormatScript, FormatUnderline, Formula,
    Note as XlsxNote, ProtectionOptions, Table as XlsxTable, TableColumn as XlsxTableColumn,
    TableFunction, TableStyle as XlsxTableStyle, Url, Worksheet,
};
use vellum_grid_core::filter::{ColumnFilter, Condition as FilterRule, Operator};
use vellum_grid_core::outline;
use vellum_grid_core::table_style::{Family, StyleId};
use vellum_grid_core::validation::{AlertStyle, Allow, Compare, Validation, literal_items};
use vellum_grid_core::{
    Cell, CellAddr, CellError, CellRange, CellValue, ChartKind, DEFAULT_FONT, DEFAULT_SIZE,
    DatePeriod, DefinedName, EN, Grouping, HAlign, Highlight, IconKind, Legend, LineStyle,
    PageSetup, PatternKind, Rule, Scale, Script, Sheet, SparkKind, Style, Threshold, TotalFunction,
    Underline, VAlign, Workbook, chart_data, formula, numfmt, sheet_name_from,
};

use crate::{Error, openformula, xlsx_format};

const SECONDS_PER_DAY: f64 = 86_400.0;
const DIGIT_PIXELS: f64 = 7.0;

pub fn read_workbook(bytes: Vec<u8>) -> Result<Workbook, Error> {
    let mut formats = xlsx_format::read(&bytes);
    let names = xlsx_format::names(&bytes);
    let manual = xlsx_format::manual_calculation(&bytes);
    let protection = crate::xlsx_review::book_protection(&bytes);
    let mut source = open_workbook_auto_from_rs(Cursor::new(bytes))?;
    let mut sheets = Vec::new();
    for (index, name) in source.sheet_names().into_iter().enumerate() {
        let title = sheet_name_from(&name).unwrap_or_else(|| format!("Sheet{}", index + 1));
        let mut sheet = Sheet::new(title);
        let values = source.worksheet_range(&name)?;
        if let Some((top, left)) = values.start() {
            for (row, col, data) in values.used_cells() {
                if let Some(cell) = convert(data) {
                    sheet.set(at(top, left, row, col)?, cell);
                }
            }
        }
        if let Ok(formulas) = source.worksheet_formula(&name)
            && let Some((top, left)) = formulas.start()
        {
            for (row, col, text) in formulas.used_cells() {
                if let Some(formula) = canonical_formula(text) {
                    let addr = at(top, left, row, col)?;
                    let mut cell = sheet.get(addr).cloned().unwrap_or_default();
                    cell.formula = Some(formula);
                    sheet.set(addr, cell);
                }
            }
        }
        if let Some(format) = formats.remove(&name) {
            for (addr, style) in format.styles {
                let mut cell = sheet.get(addr).cloned().unwrap_or_default();
                cell.set_style(style);
                sheet.set(addr, cell);
            }
            *sheet.layout_mut() = format.layout;
        }
        sheets.push(sheet);
    }
    let mut book = Workbook::from_sheets(sheets)?;
    book.set_manual_calculation(manual);
    book.protection = protection;
    for (name, scope, text, comment) in names {
        if let Some(formula) = canonical_formula(&text) {
            let defined = DefinedName {
                name,
                scope,
                formula: formula[1..].to_owned(),
                comment,
            };
            book.add_name(defined).ok();
        }
    }
    Ok(book)
}

pub fn write_xlsx(book: &Workbook) -> Result<Vec<u8>, Error> {
    let mut out = rust_xlsxwriter::Workbook::new();
    if book.manual_calculation() {
        out.set_calculation_mode(rust_xlsxwriter::CalculationMode::Manual);
    }
    let mut formats: HashMap<Style, XlsxFormat> = HashMap::new();
    let first_visible = book
        .sheets()
        .iter()
        .position(|sheet| !sheet.layout().sheet_hidden)
        .unwrap_or(0);
    for (index, sheet) in book.sheets().iter().enumerate() {
        let worksheet = out.add_worksheet();
        worksheet.set_name(sheet.name())?;
        if index == first_visible {
            worksheet.set_active(true);
        }
        let layout = sheet.layout();
        worksheet.set_screen_gridlines(!layout.hide_gridlines);
        for (col, width) in &layout.col_widths {
            worksheet
                .set_column_width_pixels(*col as u16, (width * DIGIT_PIXELS).round() as u32)?;
        }
        for (row, height) in &layout.row_heights {
            worksheet.set_row_height(*row, *height)?;
        }
        for merge in &layout.merges {
            let first = sheet.get(merge.start());
            let style = first.map_or_else(Style::default, |cell| cell.style().clone());
            let format = formats
                .entry(style.clone())
                .or_insert_with(|| xlsx_format(&style));
            let (start, end) = (merge.start(), merge.end());
            worksheet.merge_range(
                start.row(),
                start.col() as u16,
                end.row(),
                end.col() as u16,
                "",
                format,
            )?;
        }
        write_tables(worksheet, sheet)?;
        for (addr, cell) in sheet.cells() {
            let style = cell.style();
            let format = (!style.is_default()).then(|| {
                &*formats
                    .entry(style.clone())
                    .or_insert_with(|| xlsx_format(style))
            });
            match qualified(sheet, addr, cell) {
                Some(cell) => write_cell(worksheet, addr, &cell, format)?,
                None => write_cell(worksheet, addr, cell, format)?,
            }
        }
        write_view(worksheet, sheet)?;
        write_page(worksheet, &sheet.layout().page)?;
        write_rules(worksheet, sheet)?;
        write_charts(worksheet, sheet)?;
        write_sparklines(worksheet, sheet)?;
        write_drawings(worksheet, sheet)?;
        write_validations(worksheet, sheet)?;
        write_review(worksheet, sheet, index)?;
    }
    for defined in book.names() {
        let name = match defined.scope.and_then(|index| book.sheets().get(index)) {
            Some(sheet) => format!("'{}'!{}", sheet.name().replace('\'', "''"), defined.name),
            None => defined.name.clone(),
        };
        out.define_name(name, &format!("={}", defined.formula))?;
    }
    crate::xlsx_post::finish(out.save_to_buffer()?, book)
}

fn validation_rule(validation: &Validation) -> DataValidationRule<Formula> {
    let first = Formula::new(format!("={}", validation.formula1));
    let second = Formula::new(format!("={}", validation.formula2));
    match validation.compare {
        Compare::Between => DataValidationRule::Between(first, second),
        Compare::NotBetween => DataValidationRule::NotBetween(first, second),
        Compare::Equal => DataValidationRule::EqualTo(first),
        Compare::NotEqual => DataValidationRule::NotEqualTo(first),
        Compare::Greater => DataValidationRule::GreaterThan(first),
        Compare::Less => DataValidationRule::LessThan(first),
        Compare::GreaterOrEqual => DataValidationRule::GreaterThanOrEqualTo(first),
        Compare::LessOrEqual => DataValidationRule::LessThanOrEqualTo(first),
    }
}

fn write_validations(worksheet: &mut Worksheet, sheet: &Sheet) -> Result<(), Error> {
    for validation in &sheet.layout().validations {
        let Some(first) = validation.ranges.first() else {
            continue;
        };
        let base = DataValidation::new();
        let mut out = match validation.allow {
            Allow::Any => base.allow_any_value(),
            Allow::Whole => base.allow_whole_number_formula(validation_rule(validation)),
            Allow::Decimal => base.allow_decimal_number_formula(validation_rule(validation)),
            Allow::Date => base.allow_date_formula(validation_rule(validation)),
            Allow::Time => base.allow_time_formula(validation_rule(validation)),
            Allow::TextLength => base.allow_text_length_formula(validation_rule(validation)),
            Allow::Custom => base.allow_custom(Formula::new(format!("={}", validation.formula1))),
            Allow::List => match literal_items(&validation.formula1) {
                Some(items) => base.allow_list_strings(&items)?,
                None => base.allow_list_formula(Formula::new(format!("={}", validation.formula1))),
            },
        };
        out = out
            .ignore_blank(validation.ignore_blank)
            .show_dropdown(validation.dropdown)
            .show_input_message(validation.show_input)
            .show_error_message(validation.show_error)
            .set_error_style(match validation.style {
                AlertStyle::Stop => DataValidationErrorStyle::Stop,
                AlertStyle::Warning => DataValidationErrorStyle::Warning,
                AlertStyle::Information => DataValidationErrorStyle::Information,
            });
        if !validation.input_title.is_empty() {
            out = out.set_input_title(&validation.input_title)?;
        }
        if !validation.input_message.is_empty() {
            out = out.set_input_message(&validation.input_message)?;
        }
        if !validation.error_title.is_empty() {
            out = out.set_error_title(&validation.error_title)?;
        }
        if !validation.error_message.is_empty() {
            out = out.set_error_message(&validation.error_message)?;
        }
        if validation.ranges.len() > 1 {
            let all: Vec<String> = validation.ranges.iter().map(ToString::to_string).collect();
            out = out.set_multi_range(all.join(" "));
        }
        let (start, end) = (first.start(), first.end());
        worksheet.add_data_validation(
            start.row(),
            start.col() as u16,
            end.row(),
            end.col() as u16,
            &out,
        )?;
    }
    Ok(())
}

fn file_url(target: &str) -> String {
    let lower = target.to_ascii_lowercase();
    if let Some(place) = target.strip_prefix('#') {
        format!("internal:{place}")
    } else if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("ftp://")
        || lower.starts_with("mailto:")
        || lower.starts_with("file://")
    {
        target.to_owned()
    } else {
        format!("file:///{target}")
    }
}

fn write_review(worksheet: &mut Worksheet, sheet: &Sheet, index: usize) -> Result<(), Error> {
    let layout = sheet.layout();
    for (addr, note) in &layout.notes {
        let mut out = XlsxNote::new(note.text.as_str()).set_visible(note.visible);
        if !note.author.is_empty() {
            out = out.set_author(note.author.as_str());
        }
        worksheet.insert_note(addr.row(), addr.col() as u16, &out)?;
    }
    for (thread, (addr, comment)) in layout.comments.iter().enumerate() {
        let text: Vec<String> = comment
            .thread
            .iter()
            .map(|reply| format!("{}: {}", reply.author, reply.text))
            .collect();
        let author = format!("tc={}", crate::xlsx_post::comment_id(index, thread, 0));
        let note = XlsxNote::new(text.join("\n"))
            .set_author(author)
            .add_author_prefix(false);
        worksheet.insert_note(addr.row(), addr.col() as u16, &note)?;
    }
    for (addr, link) in &layout.links {
        let shown = sheet
            .get(*addr)
            .map_or_else(String::new, |cell| match &cell.value {
                CellValue::Empty => String::new(),
                value => EN.display(value),
            });
        let mut url = Url::new(file_url(&link.target));
        if !shown.is_empty() {
            url = url.set_text(shown);
        }
        if !link.tooltip.is_empty() {
            url = url.set_tip(link.tooltip.as_str());
        }
        let style = sheet
            .get(*addr)
            .map(|cell| cell.style().clone())
            .unwrap_or_default();
        worksheet.write_url_with_format(
            addr.row(),
            addr.col() as u16,
            url,
            &xlsx_format(&style),
        )?;
    }
    if let Some(protection) = &layout.protection {
        let allow = protection.allow;
        let options = ProtectionOptions {
            select_locked_cells: allow.select_locked,
            select_unlocked_cells: allow.select_unlocked,
            format_cells: allow.format_cells,
            format_columns: allow.format_columns,
            format_rows: allow.format_rows,
            insert_columns: allow.insert_columns,
            insert_rows: allow.insert_rows,
            insert_links: allow.insert_links,
            delete_columns: allow.delete_columns,
            delete_rows: allow.delete_rows,
            sort: allow.sort,
            use_autofilter: allow.autofilter,
            edit_objects: allow.edit_objects,
            ..ProtectionOptions::default()
        };
        worksheet.protect_with_options(&options);
    }
    Ok(())
}

const LIGHT_STYLES: [XlsxTableStyle; 21] = [
    XlsxTableStyle::Light1,
    XlsxTableStyle::Light2,
    XlsxTableStyle::Light3,
    XlsxTableStyle::Light4,
    XlsxTableStyle::Light5,
    XlsxTableStyle::Light6,
    XlsxTableStyle::Light7,
    XlsxTableStyle::Light8,
    XlsxTableStyle::Light9,
    XlsxTableStyle::Light10,
    XlsxTableStyle::Light11,
    XlsxTableStyle::Light12,
    XlsxTableStyle::Light13,
    XlsxTableStyle::Light14,
    XlsxTableStyle::Light15,
    XlsxTableStyle::Light16,
    XlsxTableStyle::Light17,
    XlsxTableStyle::Light18,
    XlsxTableStyle::Light19,
    XlsxTableStyle::Light20,
    XlsxTableStyle::Light21,
];
const MEDIUM_STYLES: [XlsxTableStyle; 28] = [
    XlsxTableStyle::Medium1,
    XlsxTableStyle::Medium2,
    XlsxTableStyle::Medium3,
    XlsxTableStyle::Medium4,
    XlsxTableStyle::Medium5,
    XlsxTableStyle::Medium6,
    XlsxTableStyle::Medium7,
    XlsxTableStyle::Medium8,
    XlsxTableStyle::Medium9,
    XlsxTableStyle::Medium10,
    XlsxTableStyle::Medium11,
    XlsxTableStyle::Medium12,
    XlsxTableStyle::Medium13,
    XlsxTableStyle::Medium14,
    XlsxTableStyle::Medium15,
    XlsxTableStyle::Medium16,
    XlsxTableStyle::Medium17,
    XlsxTableStyle::Medium18,
    XlsxTableStyle::Medium19,
    XlsxTableStyle::Medium20,
    XlsxTableStyle::Medium21,
    XlsxTableStyle::Medium22,
    XlsxTableStyle::Medium23,
    XlsxTableStyle::Medium24,
    XlsxTableStyle::Medium25,
    XlsxTableStyle::Medium26,
    XlsxTableStyle::Medium27,
    XlsxTableStyle::Medium28,
];
const DARK_STYLES: [XlsxTableStyle; 11] = [
    XlsxTableStyle::Dark1,
    XlsxTableStyle::Dark2,
    XlsxTableStyle::Dark3,
    XlsxTableStyle::Dark4,
    XlsxTableStyle::Dark5,
    XlsxTableStyle::Dark6,
    XlsxTableStyle::Dark7,
    XlsxTableStyle::Dark8,
    XlsxTableStyle::Dark9,
    XlsxTableStyle::Dark10,
    XlsxTableStyle::Dark11,
];

fn table_style(name: &str) -> XlsxTableStyle {
    let Some(id) = StyleId::parse(name) else {
        return XlsxTableStyle::None;
    };
    let index = usize::from(id.number - 1);
    match id.family {
        Family::Light => LIGHT_STYLES[index],
        Family::Medium => MEDIUM_STYLES[index],
        Family::Dark => DARK_STYLES[index],
    }
}

fn write_tables(worksheet: &mut Worksheet, sheet: &Sheet) -> Result<(), Error> {
    for table in &sheet.layout().tables {
        let columns: Vec<XlsxTableColumn> = table
            .columns
            .iter()
            .map(|column| {
                let function = match column.total {
                    TotalFunction::Average => TableFunction::Average,
                    TotalFunction::Count => TableFunction::Count,
                    TotalFunction::CountNums => TableFunction::CountNumbers,
                    TotalFunction::Max => TableFunction::Max,
                    TotalFunction::Min => TableFunction::Min,
                    TotalFunction::Sum => TableFunction::Sum,
                    TotalFunction::StdDev => TableFunction::StdDev,
                    TotalFunction::Var => TableFunction::Var,
                    TotalFunction::None | TotalFunction::Custom => TableFunction::None,
                };
                let mut out = XlsxTableColumn::new()
                    .set_header(column.name.as_str())
                    .set_total_function(function);
                if let Some(label) = &column.label {
                    out = out.set_total_label(label.as_str());
                }
                out
            })
            .collect();
        let out = XlsxTable::new()
            .set_name(table.name.as_str())
            .set_style(table_style(&table.style))
            .set_header_row(table.header)
            .set_total_row(table.totals)
            .set_banded_rows(table.banded_rows)
            .set_banded_columns(table.banded_cols)
            .set_first_column(table.first_col)
            .set_last_column(table.last_col)
            .set_autofilter(table.filter && table.header)
            .set_columns(&columns);
        let (start, end) = (table.range.start(), table.range.end());
        worksheet.add_table(
            start.row(),
            start.col() as u16,
            end.row(),
            end.col() as u16,
            &out,
        )?;
    }
    Ok(())
}

fn qualified(sheet: &Sheet, addr: CellAddr, cell: &Cell) -> Option<Cell> {
    let text = cell.formula.as_deref()?;
    let table = sheet
        .layout()
        .tables
        .iter()
        .find(|t| t.range.contains(addr))?;
    let expr = formula::parse(&text[1..], &EN).ok()?;
    let named = expr.map_structured(&|reference| {
        reference.table.is_none().then(|| {
            formula::Expr::Structured(formula::StructuredRef {
                table: Some(table.name.clone()),
                ..reference.clone()
            })
        })
    });
    (named != expr).then(|| Cell {
        formula: Some(format!("={}", named.write(&EN))),
        ..cell.clone()
    })
}

fn write_view(worksheet: &mut Worksheet, sheet: &Sheet) -> Result<(), Error> {
    let layout = sheet.layout();
    if let Some(color) = layout.tab_color {
        worksheet.set_tab_color(XlsxColor::RGB(color.0));
    }
    if layout.sheet_hidden {
        worksheet.set_hidden(true);
    }
    if layout.freeze_rows > 0 || layout.freeze_cols > 0 {
        worksheet.set_freeze_panes(layout.freeze_rows, layout.freeze_cols as u16)?;
    }
    for row in &layout.hidden_rows {
        worksheet.set_row_hidden(*row)?;
    }
    for col in &layout.hidden_cols {
        worksheet.set_column_hidden(*col as u16)?;
    }
    for group in outline::groups(&layout.row_levels) {
        if layout.collapsed_rows.contains(&group.end) {
            worksheet.group_rows_collapsed(group.start, group.end)?;
        } else {
            worksheet.group_rows(group.start, group.end)?;
        }
    }
    for group in outline::groups(&layout.col_levels) {
        let (start, end) = (group.start as u16, group.end as u16);
        if layout.collapsed_cols.contains(&group.end) {
            worksheet.group_columns_collapsed(start, end)?;
        } else {
            worksheet.group_columns(start, end)?;
        }
    }
    let tabled = |range: CellRange| layout.tables.iter().any(|t| t.filter_range() == range);
    if let Some(filter) = layout.filter.as_ref().filter(|f| !tabled(f.range)) {
        let (start, end) = (filter.range.start(), filter.range.end());
        worksheet.autofilter(start.row(), start.col() as u16, end.row(), end.col() as u16)?;
        worksheet.filter_automatic_off();
        for (col, criteria) in &filter.criteria {
            if let Some(condition) = filter_condition(criteria) {
                worksheet.filter_column(*col as u16, &condition)?;
            }
        }
    }
    Ok(())
}

fn filter_condition(criteria: &ColumnFilter) -> Option<FilterCondition> {
    match criteria {
        ColumnFilter::Values(shown) => Some(shown.iter().fold(
            FilterCondition::new(),
            |condition, value| {
                if value.is_empty() {
                    condition.add_list_blanks_filter()
                } else {
                    condition.add_list_filter(value.as_str())
                }
            },
        )),
        ColumnFilter::Custom { first, second, and } => {
            let add = |condition: FilterCondition, rule: &FilterRule| {
                let criteria = match rule.op {
                    Operator::Equal => FilterCriteria::EqualTo,
                    Operator::NotEqual => FilterCriteria::NotEqualTo,
                    Operator::Greater => FilterCriteria::GreaterThan,
                    Operator::GreaterOrEqual => FilterCriteria::GreaterThanOrEqualTo,
                    Operator::Less => FilterCriteria::LessThan,
                    Operator::LessOrEqual => FilterCriteria::LessThanOrEqualTo,
                    Operator::BeginsWith => FilterCriteria::BeginsWith,
                    Operator::NotBeginsWith => FilterCriteria::DoesNotBeginWith,
                    Operator::EndsWith => FilterCriteria::EndsWith,
                    Operator::NotEndsWith => FilterCriteria::DoesNotEndWith,
                    Operator::Contains => FilterCriteria::Contains,
                    Operator::NotContains => FilterCriteria::DoesNotContain,
                };
                match EN.parse_number(&rule.value) {
                    Some(number) => condition.add_custom_filter(criteria, number),
                    None => condition.add_custom_filter(criteria, rule.value.as_str()),
                }
            };
            let mut condition = add(FilterCondition::new(), first);
            if let Some(second) = second {
                if !and {
                    condition = condition.add_custom_boolean_or();
                }
                condition = add(condition, second);
            }
            Some(condition)
        }
        _ => None,
    }
}

fn write_page(worksheet: &mut Worksheet, setup: &PageSetup) -> Result<(), Error> {
    if *setup == PageSetup::default() {
        return Ok(());
    }
    worksheet.set_paper_size(setup.paper.code());
    if setup.landscape {
        worksheet.set_landscape();
    }
    let m = setup.margins;
    worksheet.set_margins(m.left, m.right, m.top, m.bottom, m.header, m.footer);
    match setup.scale {
        Scale::Percent(100) => {}
        Scale::Percent(percent) => {
            worksheet.set_print_scale(percent);
        }
        Scale::Fit { width, height } => {
            worksheet.set_print_fit_to_pages(width, height);
        }
    }
    if let Some(area) = setup.print_area {
        let (start, end) = (area.start(), area.end());
        worksheet.set_print_area(start.row(), start.col() as u16, end.row(), end.col() as u16)?;
    }
    if let Some((first, last)) = setup.title_rows {
        worksheet.set_repeat_rows(first, last)?;
    }
    if let Some((first, last)) = setup.title_cols {
        worksheet.set_repeat_columns(first as u16, last as u16)?;
    }
    if !setup.header.is_empty() {
        worksheet.set_header(setup.header.as_str());
    }
    if !setup.footer.is_empty() {
        worksheet.set_footer(setup.footer.as_str());
    }
    worksheet.set_print_gridlines(setup.gridlines);
    worksheet.set_print_headings(setup.headings);
    worksheet.set_print_center_horizontally(setup.center_horizontally);
    worksheet.set_print_center_vertically(setup.center_vertically);
    let rows: Vec<u32> = setup.row_breaks.iter().copied().collect();
    worksheet.set_page_breaks(&rows)?;
    let cols: Vec<u32> = setup.col_breaks.iter().copied().collect();
    worksheet.set_vertical_page_breaks(&cols)?;
    Ok(())
}

fn xlsx_line(style: LineStyle) -> FormatBorder {
    match style {
        LineStyle::Hair => FormatBorder::Hair,
        LineStyle::Dotted => FormatBorder::Dotted,
        LineStyle::DashDotDot => FormatBorder::DashDotDot,
        LineStyle::DashDot => FormatBorder::DashDot,
        LineStyle::Dashed => FormatBorder::Dashed,
        LineStyle::Thin => FormatBorder::Thin,
        LineStyle::MediumDashDotDot => FormatBorder::MediumDashDotDot,
        LineStyle::SlantDashDot => FormatBorder::SlantDashDot,
        LineStyle::MediumDashDot => FormatBorder::MediumDashDot,
        LineStyle::MediumDashed => FormatBorder::MediumDashed,
        LineStyle::Medium => FormatBorder::Medium,
        LineStyle::Thick => FormatBorder::Thick,
        LineStyle::Double => FormatBorder::Double,
    }
}

fn cf_value(threshold: Threshold) -> (ConditionalFormatType, f64) {
    match threshold {
        Threshold::Automatic => (ConditionalFormatType::Automatic, 0.0),
        Threshold::Lowest => (ConditionalFormatType::Lowest, 0.0),
        Threshold::Highest => (ConditionalFormatType::Highest, 0.0),
        Threshold::Number(n) => (ConditionalFormatType::Number, n),
        Threshold::Percent(n) => (ConditionalFormatType::Percent, n),
        Threshold::Percentile(n) => (ConditionalFormatType::Percentile, n),
    }
}

fn icon_type(kind: IconKind) -> ConditionalFormatIconType {
    match kind {
        IconKind::Arrows3 => ConditionalFormatIconType::ThreeArrows,
        IconKind::ArrowsGray3 => ConditionalFormatIconType::ThreeArrowsGray,
        IconKind::Flags3 => ConditionalFormatIconType::ThreeFlags,
        IconKind::TrafficLights3 => ConditionalFormatIconType::ThreeTrafficLights,
        IconKind::TrafficLightsRim3 => ConditionalFormatIconType::ThreeTrafficLightsWithRim,
        IconKind::Signs3 => ConditionalFormatIconType::ThreeSigns,
        IconKind::SymbolsCircled3 => ConditionalFormatIconType::ThreeSymbolsCircled,
        IconKind::Symbols3 => ConditionalFormatIconType::ThreeSymbols,
        IconKind::Stars3 => ConditionalFormatIconType::ThreeStars,
        IconKind::Triangles3 => ConditionalFormatIconType::ThreeTriangles,
        IconKind::Arrows4 => ConditionalFormatIconType::FourArrows,
        IconKind::ArrowsGray4 => ConditionalFormatIconType::FourArrowsGray,
        IconKind::RedToBlack4 => ConditionalFormatIconType::FourRedToBlack,
        IconKind::Rating4 => ConditionalFormatIconType::FourHistograms,
        IconKind::TrafficLights4 => ConditionalFormatIconType::FourTrafficLights,
        IconKind::Arrows5 => ConditionalFormatIconType::FiveArrows,
        IconKind::ArrowsGray5 => ConditionalFormatIconType::FiveArrowsGray,
        IconKind::Rating5 => ConditionalFormatIconType::FiveHistograms,
        IconKind::Quarters5 => ConditionalFormatIconType::FiveQuadrants,
        IconKind::Boxes5 => ConditionalFormatIconType::FiveBoxes,
    }
}

fn highlight_format(highlight: &Highlight) -> XlsxFormat {
    let mut style = XlsxFormat::new();
    if let Some(fill) = highlight.fill {
        style = style.set_background_color(XlsxColor::RGB(fill.0));
    }
    if let Some(color) = highlight.color {
        style = style.set_font_color(XlsxColor::RGB(color.0));
    }
    if highlight.bold {
        style = style.set_bold();
    }
    if highlight.italic {
        style = style.set_italic();
    }
    if highlight.strike {
        style = style.set_font_strikethrough();
    }
    if let Some(border) = highlight.border {
        style = style
            .set_border(xlsx_line(border.style))
            .set_border_color(XlsxColor::RGB(border.color.0));
    }
    style
}

fn literal(value: &CellValue) -> Option<String> {
    match value {
        CellValue::Number(n) => Some(EN.display(&CellValue::Number(*n))),
        CellValue::Text(text) => Some(format!("\"{}\"", text.replace('"', "\"\""))),
        CellValue::Bool(b) => Some(EN.display(&CellValue::Bool(*b))),
        _ => None,
    }
}

fn write_rules(worksheet: &mut Worksheet, sheet: &Sheet) -> Result<(), Error> {
    macro_rules! add {
        ($range:expr, $rule:expr) => {{
            let (r1, c1, r2, c2) = $range;
            worksheet.add_conditional_format(r1, c1, r2, c2, &$rule)?;
        }};
    }
    for format in sheet.layout().conditional.iter().rev() {
        let style = highlight_format(&format.format);
        let (start, end) = (format.range.start(), format.range.end());
        let span = (start.row(), start.col() as u16, end.row(), end.col() as u16);
        let stop = format.stop;
        let cell = |rule: ConditionalFormatCellRule<f64>| {
            ConditionalFormatCell::new()
                .set_rule(rule)
                .set_format(&style)
                .set_stop_if_true(stop)
        };
        let compare = |rule: ConditionalFormatCellRule<String>| {
            ConditionalFormatCell::new()
                .set_rule(rule)
                .set_format(&style)
                .set_stop_if_true(stop)
        };
        let text = |rule: ConditionalFormatTextRule| {
            ConditionalFormatText::new()
                .set_rule(rule)
                .set_format(&style)
                .set_stop_if_true(stop)
        };
        match &format.rule {
            Rule::Greater(v) => add!(span, cell(ConditionalFormatCellRule::GreaterThan(*v))),
            Rule::Less(v) => add!(span, cell(ConditionalFormatCellRule::LessThan(*v))),
            Rule::GreaterEqual(v) => {
                add!(
                    span,
                    cell(ConditionalFormatCellRule::GreaterThanOrEqualTo(*v))
                )
            }
            Rule::LessEqual(v) => {
                add!(span, cell(ConditionalFormatCellRule::LessThanOrEqualTo(*v)))
            }
            Rule::Between(a, b) => add!(
                span,
                cell(ConditionalFormatCellRule::Between(a.min(*b), a.max(*b),))
            ),
            Rule::NotBetween(a, b) => add!(
                span,
                cell(ConditionalFormatCellRule::NotBetween(a.min(*b), a.max(*b),))
            ),
            Rule::Equal(CellValue::Number(n)) => {
                add!(span, cell(ConditionalFormatCellRule::EqualTo(*n)))
            }
            Rule::NotEqual(CellValue::Number(n)) => {
                add!(span, cell(ConditionalFormatCellRule::NotEqualTo(*n)))
            }
            Rule::Equal(value) => match literal(value) {
                Some(v) => add!(span, compare(ConditionalFormatCellRule::EqualTo(v))),
                None => continue,
            },
            Rule::NotEqual(value) => match literal(value) {
                Some(v) => add!(span, compare(ConditionalFormatCellRule::NotEqualTo(v))),
                None => continue,
            },
            Rule::TextContains(t) => {
                add!(span, text(ConditionalFormatTextRule::Contains(t.clone())))
            }
            Rule::TextNotContains(t) => {
                add!(
                    span,
                    text(ConditionalFormatTextRule::DoesNotContain(t.clone()))
                )
            }
            Rule::BeginsWith(t) => {
                add!(span, text(ConditionalFormatTextRule::BeginsWith(t.clone())))
            }
            Rule::EndsWith(t) => add!(span, text(ConditionalFormatTextRule::EndsWith(t.clone()))),
            Rule::Duplicate | Rule::Unique => {
                let mut rule = ConditionalFormatDuplicate::new()
                    .set_format(&style)
                    .set_stop_if_true(stop);
                if format.rule == Rule::Unique {
                    rule = rule.invert();
                }
                add!(span, rule)
            }
            Rule::Blanks | Rule::NoBlanks => {
                let mut rule = ConditionalFormatBlank::new()
                    .set_format(&style)
                    .set_stop_if_true(stop);
                if format.rule == Rule::NoBlanks {
                    rule = rule.invert();
                }
                add!(span, rule)
            }
            Rule::Errors | Rule::NoErrors => {
                let mut rule = ConditionalFormatError::new()
                    .set_format(&style)
                    .set_stop_if_true(stop);
                if format.rule == Rule::NoErrors {
                    rule = rule.invert();
                }
                add!(span, rule)
            }
            Rule::Dates(period) => {
                let criteria = match period {
                    DatePeriod::Yesterday => ConditionalFormatDateRule::Yesterday,
                    DatePeriod::Today => ConditionalFormatDateRule::Today,
                    DatePeriod::Tomorrow => ConditionalFormatDateRule::Tomorrow,
                    DatePeriod::Last7Days => ConditionalFormatDateRule::Last7Days,
                    DatePeriod::LastWeek => ConditionalFormatDateRule::LastWeek,
                    DatePeriod::ThisWeek => ConditionalFormatDateRule::ThisWeek,
                    DatePeriod::NextWeek => ConditionalFormatDateRule::NextWeek,
                    DatePeriod::LastMonth => ConditionalFormatDateRule::LastMonth,
                    DatePeriod::ThisMonth => ConditionalFormatDateRule::ThisMonth,
                    DatePeriod::NextMonth => ConditionalFormatDateRule::NextMonth,
                };
                add!(
                    span,
                    ConditionalFormatDate::new()
                        .set_rule(criteria)
                        .set_format(&style)
                        .set_stop_if_true(stop)
                )
            }
            Rule::Top {
                count,
                percent,
                bottom,
            } => {
                let n = (*count).min(u32::from(u16::MAX)) as u16;
                let criteria = match (percent, bottom) {
                    (false, false) => ConditionalFormatTopRule::Top(n),
                    (false, true) => ConditionalFormatTopRule::Bottom(n),
                    (true, false) => ConditionalFormatTopRule::TopPercent(n),
                    (true, true) => ConditionalFormatTopRule::BottomPercent(n),
                };
                add!(
                    span,
                    ConditionalFormatTop::new()
                        .set_rule(criteria)
                        .set_format(&style)
                        .set_stop_if_true(stop)
                )
            }
            Rule::Average {
                below,
                equal,
                deviations,
            } => {
                use ConditionalFormatAverageRule as A;
                let criteria = match (below, equal, deviations) {
                    (false, false, 0) => A::AboveAverage,
                    (true, false, 0) => A::BelowAverage,
                    (false, true, _) => A::EqualOrAboveAverage,
                    (true, true, _) => A::EqualOrBelowAverage,
                    (false, _, 1) => A::OneStandardDeviationAbove,
                    (true, _, 1) => A::OneStandardDeviationBelow,
                    (false, _, 2) => A::TwoStandardDeviationsAbove,
                    (true, _, 2) => A::TwoStandardDeviationsBelow,
                    (false, _, _) => A::ThreeStandardDeviationsAbove,
                    (true, _, _) => A::ThreeStandardDeviationsBelow,
                };
                add!(
                    span,
                    ConditionalFormatAverage::new()
                        .set_rule(criteria)
                        .set_format(&style)
                        .set_stop_if_true(stop)
                )
            }
            Rule::Expression(text) => add!(
                span,
                ConditionalFormatFormula::new()
                    .set_rule(format!("={text}").as_str())
                    .set_format(&style)
                    .set_stop_if_true(stop)
            ),
            Rule::DataBar(bar) => {
                let (min_type, min) = cf_value(bar.min);
                let (max_type, max) = cf_value(bar.max);
                add!(
                    span,
                    ConditionalFormatDataBar::new()
                        .set_minimum(min_type, min)
                        .set_maximum(max_type, max)
                        .set_fill_color(XlsxColor::RGB(bar.color.0))
                        .set_solid_fill(!bar.gradient)
                        .set_bar_only(!bar.show_value)
                )
            }
            Rule::ColorScale(scale) => match scale.points.as_slice() {
                [(a, ca), (b, cb)] => {
                    let ((ta, va), (tb, vb)) = (cf_value(*a), cf_value(*b));
                    add!(
                        span,
                        ConditionalFormat2ColorScale::new()
                            .set_minimum(ta, va)
                            .set_maximum(tb, vb)
                            .set_minimum_color(XlsxColor::RGB(ca.0))
                            .set_maximum_color(XlsxColor::RGB(cb.0))
                    )
                }
                [(a, ca), (m, cm), (b, cb)] => {
                    let ((ta, va), (tm, vm), (tb, vb)) = (cf_value(*a), cf_value(*m), cf_value(*b));
                    add!(
                        span,
                        ConditionalFormat3ColorScale::new()
                            .set_minimum(ta, va)
                            .set_midpoint(tm, vm)
                            .set_maximum(tb, vb)
                            .set_minimum_color(XlsxColor::RGB(ca.0))
                            .set_midpoint_color(XlsxColor::RGB(cm.0))
                            .set_maximum_color(XlsxColor::RGB(cb.0))
                    )
                }
                _ => continue,
            },
            Rule::IconSet(set) => {
                let mut rule = ConditionalFormatIconSet::new()
                    .set_icon_type(icon_type(set.kind))
                    .reverse_icons(set.reverse)
                    .show_icons_only(!set.show_value);
                if set.thresholds != set.kind.default_thresholds() {
                    let mut icons = vec![
                        ConditionalFormatCustomIcon::new()
                            .set_rule(ConditionalFormatType::Percent, 0.0),
                    ];
                    icons.extend(set.thresholds.iter().map(|t| {
                        let (kind, value) = cf_value(*t);
                        ConditionalFormatCustomIcon::new().set_rule(kind, value)
                    }));
                    rule = rule.set_icons(&icons);
                }
                add!(span, rule)
            }
        };
    }
    Ok(())
}

fn write_charts(worksheet: &mut Worksheet, sheet: &Sheet) -> Result<(), Error> {
    let name = sheet.name();
    let span = |range: CellRange| {
        let (start, end) = (range.start(), range.end());
        (
            name,
            start.row(),
            start.col() as u16,
            end.row(),
            end.col() as u16,
        )
    };
    for chart in &sheet.layout().charts {
        let data = chart_data(sheet, chart.data, &EN);
        use rust_xlsxwriter::ChartType as T;
        let kind = match (chart.kind, chart.grouping) {
            (ChartKind::Column, Grouping::Clustered) => T::Column,
            (ChartKind::Column, Grouping::Stacked) => T::ColumnStacked,
            (ChartKind::Column, Grouping::Percent) => T::ColumnPercentStacked,
            (ChartKind::Bar, Grouping::Clustered) => T::Bar,
            (ChartKind::Bar, Grouping::Stacked) => T::BarStacked,
            (ChartKind::Bar, Grouping::Percent) => T::BarPercentStacked,
            (ChartKind::Line, Grouping::Clustered) => T::Line,
            (ChartKind::Line, Grouping::Stacked) => T::LineStacked,
            (ChartKind::Line, Grouping::Percent) => T::LinePercentStacked,
            (ChartKind::Area, Grouping::Clustered) => T::Area,
            (ChartKind::Area, Grouping::Stacked) => T::AreaStacked,
            (ChartKind::Area, Grouping::Percent) => T::AreaPercentStacked,
            (ChartKind::Pie, _) => T::Pie,
            (ChartKind::Doughnut, _) => T::Doughnut,
            (ChartKind::Scatter, _) => T::Scatter,
        };
        let mut out = XlsxChart::new(kind);
        let scatter_x = (chart.kind == ChartKind::Scatter
            && data.categories_range.is_none()
            && data.series.len() > 1)
            .then(|| data.series[0].range);
        let categories = scatter_x.or(data.categories_range);
        let skip = usize::from(scatter_x.is_some());
        let count = if chart.kind.is_round() { 1 } else { usize::MAX };
        for series in data.series.iter().skip(skip).take(count) {
            let item = out.add_series();
            item.set_values(span(series.range));
            if let Some(range) = categories {
                item.set_categories(span(range));
            }
            match series.name_cell {
                Some(cell) => item.set_name((name, cell.row(), cell.col() as u16)),
                None => item.set_name(series.name.as_str()),
            };
            if chart.labels {
                item.set_data_label(rust_xlsxwriter::ChartDataLabel::new().show_value());
            }
            if chart.kind == ChartKind::Line {
                let mut marker = rust_xlsxwriter::ChartMarker::new();
                if chart.markers {
                    marker.set_automatic();
                } else {
                    marker.set_none();
                }
                item.set_marker(&marker);
            }
        }
        if let Some(title) = &chart.title {
            out.title().set_name(title.as_str());
        }
        use rust_xlsxwriter::ChartLegendPosition as P;
        match chart.legend {
            Legend::Auto if !chart.legend_shown(&data) => {
                out.legend().set_hidden();
            }
            Legend::Hidden => {
                out.legend().set_hidden();
            }
            Legend::Auto | Legend::Bottom => {
                out.legend().set_position(P::Bottom);
            }
            Legend::Top => {
                out.legend().set_position(P::Top);
            }
            Legend::Left => {
                out.legend().set_position(P::Left);
            }
            Legend::Right => {
                out.legend().set_position(P::Right);
            }
        }
        if !chart.kind.is_round() {
            if let Some(title) = &chart.x_title {
                out.x_axis().set_name(title.as_str());
            }
            if let Some(title) = &chart.y_title {
                out.y_axis().set_name(title.as_str());
            }
            out.y_axis().set_major_gridlines(chart.gridlines);
        }
        out.set_width(chart.size.0).set_height(chart.size.1);
        worksheet.insert_chart_with_offset(
            chart.anchor.row(),
            chart.anchor.col() as u16,
            &out,
            chart.offset.0,
            chart.offset.1,
        )?;
    }
    Ok(())
}

fn write_drawings(worksheet: &mut Worksheet, sheet: &Sheet) -> Result<(), Error> {
    let layout = sheet.layout();
    for (index, picture) in layout.pictures.iter().enumerate() {
        let Ok(image) = rust_xlsxwriter::Image::new_from_buffer(&picture.data) else {
            continue;
        };
        let image = image
            .set_scale_to_size(picture.size.0, picture.size.1, false)
            .set_alt_text(format!("{}{index}", crate::xlsx_drawing::PICTURE_MARKER));
        worksheet.insert_image(index as u32, 0, &image)?;
    }
    if !layout.shapes.is_empty() {
        worksheet.insert_shape(0, 0, &rust_xlsxwriter::Shape::textbox())?;
    }
    Ok(())
}

fn write_sparklines(worksheet: &mut Worksheet, sheet: &Sheet) -> Result<(), Error> {
    let name = sheet.name();
    let span = |range: CellRange| {
        let (start, end) = (range.start(), range.end());
        (
            name,
            start.row(),
            start.col() as u16,
            end.row(),
            end.col() as u16,
        )
    };
    for group in sheet.layout().sparklines.iter().rev() {
        let Some((first, rest)) = group.cells.split_first() else {
            continue;
        };
        let union = |pick: &dyn Fn(&(CellAddr, CellRange)) -> CellRange| {
            rest.iter().fold(pick(first), |a, b| {
                a.including(pick(b).start()).including(pick(b).end())
            })
        };
        let locations = union(&|(at, _)| CellRange::single(*at));
        let data = union(&|(_, range)| *range);
        let base = rust_xlsxwriter::Sparkline::new()
            .set_type(match group.kind {
                SparkKind::Line => rust_xlsxwriter::SparklineType::Line,
                SparkKind::Column => rust_xlsxwriter::SparklineType::Column,
                SparkKind::WinLoss => rust_xlsxwriter::SparklineType::WinLose,
            })
            .set_sparkline_color(XlsxColor::RGB(group.color.0))
            .show_markers(group.markers)
            .show_high_point(group.high)
            .show_low_point(group.low)
            .show_first_point(group.first)
            .show_last_point(group.last)
            .show_negative_points(group.negative);
        let point = XlsxColor::RGB(group.point_color.0);
        let mut base = base;
        for (on, set) in [
            (
                group.high,
                rust_xlsxwriter::Sparkline::set_high_point_color as fn(_, _) -> _,
            ),
            (group.low, rust_xlsxwriter::Sparkline::set_low_point_color),
            (
                group.first,
                rust_xlsxwriter::Sparkline::set_first_point_color,
            ),
            (group.last, rust_xlsxwriter::Sparkline::set_last_point_color),
            (
                group.negative,
                rust_xlsxwriter::Sparkline::set_negative_points_color,
            ),
            (group.markers, rust_xlsxwriter::Sparkline::set_markers_color),
        ] {
            if on {
                base = set(base, point);
            }
        }
        let grouped = group.cells.len() > 1
            && vellum_grid_core::spark_pairs(data, locations).as_deref() == Some(&group.cells[..]);
        if grouped {
            let (start, end) = (locations.start(), locations.end());
            worksheet.add_sparkline_group(
                start.row(),
                start.col() as u16,
                end.row(),
                end.col() as u16,
                &base.set_range(span(data)),
            )?;
        } else {
            for (at, range) in &group.cells {
                worksheet.add_sparkline(
                    at.row(),
                    at.col() as u16,
                    &base.clone().set_range(span(*range)),
                )?;
            }
        }
    }
    Ok(())
}

fn write_cell(
    worksheet: &mut Worksheet,
    addr: CellAddr,
    cell: &Cell,
    format: Option<&XlsxFormat>,
) -> Result<(), Error> {
    let (row, col) = (addr.row(), addr.col() as u16);
    if let Some(text) = &cell.formula {
        let formula = Formula::new(text).set_result(EN.display(&cell.value));
        match format {
            Some(format) => worksheet.write_formula_with_format(row, col, formula, format)?,
            None => worksheet.write_formula(row, col, formula)?,
        };
        return Ok(());
    }
    match (&cell.value, format) {
        (CellValue::Number(n), Some(f)) => worksheet.write_number_with_format(row, col, *n, f)?,
        (CellValue::Number(n), None) => worksheet.write_number(row, col, *n)?,
        (CellValue::Text(s), Some(f)) => worksheet.write_string_with_format(row, col, s, f)?,
        (CellValue::Text(s), None) => worksheet.write_string(row, col, s)?,
        (CellValue::Bool(b), Some(f)) => worksheet.write_boolean_with_format(row, col, *b, f)?,
        (CellValue::Bool(b), None) => worksheet.write_boolean(row, col, *b)?,
        (CellValue::Error(e), _) => {
            let formula = Formula::new(format!("={}", e.code())).set_result(e.code());
            match format {
                Some(f) => worksheet.write_formula_with_format(row, col, formula, f)?,
                None => worksheet.write_formula(row, col, formula)?,
            }
        }
        (CellValue::Empty, Some(f)) => worksheet.write_blank(row, col, f)?,
        (CellValue::Empty, None) => return Ok(()),
    };
    Ok(())
}

fn xlsx_format(style: &Style) -> XlsxFormat {
    let mut format = XlsxFormat::new();
    if !style.has_general_format() {
        format = format.set_num_format(&style.number_format);
    }
    if style.font != DEFAULT_FONT {
        format = format.set_font_name(&style.font);
    }
    if style.size != DEFAULT_SIZE {
        format = format.set_font_size(style.points());
    }
    if style.bold {
        format = format.set_bold();
    }
    if style.italic {
        format = format.set_italic();
    }
    format = match style.underline {
        Underline::None => format,
        Underline::Single => format.set_underline(FormatUnderline::Single),
        Underline::Double => format.set_underline(FormatUnderline::Double),
        Underline::SingleAccounting => format.set_underline(FormatUnderline::SingleAccounting),
        Underline::DoubleAccounting => format.set_underline(FormatUnderline::DoubleAccounting),
    };
    if style.strike {
        format = format.set_font_strikethrough();
    }
    format = match style.script {
        Script::None => format,
        Script::Super => format.set_font_script(FormatScript::Superscript),
        Script::Sub => format.set_font_script(FormatScript::Subscript),
    };
    if let Some(color) = style.color {
        format = format.set_font_color(XlsxColor::RGB(color.0));
    }
    match (style.pattern, style.fill) {
        (Some(pattern), fill) => {
            format = format
                .set_pattern(xlsx_pattern(pattern.kind))
                .set_foreground_color(XlsxColor::RGB(pattern.color.0));
            if let Some(fill) = fill {
                format = format.set_background_color(XlsxColor::RGB(fill.0));
            }
        }
        (None, Some(fill)) => {
            format = format
                .set_pattern(FormatPattern::Solid)
                .set_background_color(XlsxColor::RGB(fill.0));
        }
        (None, None) => {}
    }
    let line = xlsx_line;
    let borders = style.borders;
    if let Some(b) = borders.diagonal {
        let kind = match (borders.diagonal_up, borders.diagonal_down) {
            (true, true) => FormatDiagonalBorder::BorderUpDown,
            (true, false) => FormatDiagonalBorder::BorderUp,
            (false, true) => FormatDiagonalBorder::BorderDown,
            (false, false) => FormatDiagonalBorder::None,
        };
        if kind != FormatDiagonalBorder::None {
            format = format
                .set_border_diagonal(line(b.style))
                .set_border_diagonal_color(XlsxColor::RGB(b.color.0))
                .set_border_diagonal_type(kind);
        }
    }
    if let Some(b) = borders.top {
        format = format
            .set_border_top(line(b.style))
            .set_border_top_color(XlsxColor::RGB(b.color.0));
    }
    if let Some(b) = borders.bottom {
        format = format
            .set_border_bottom(line(b.style))
            .set_border_bottom_color(XlsxColor::RGB(b.color.0));
    }
    if let Some(b) = borders.left {
        format = format
            .set_border_left(line(b.style))
            .set_border_left_color(XlsxColor::RGB(b.color.0));
    }
    if let Some(b) = borders.right {
        format = format
            .set_border_right(line(b.style))
            .set_border_right_color(XlsxColor::RGB(b.color.0));
    }
    format = match style.h_align {
        HAlign::General => format,
        HAlign::Left => format.set_align(FormatAlign::Left),
        HAlign::Center => format.set_align(FormatAlign::Center),
        HAlign::Right => format.set_align(FormatAlign::Right),
        HAlign::Fill => format.set_align(FormatAlign::Fill),
        HAlign::Justify => format.set_align(FormatAlign::Justify),
        HAlign::CenterAcross => format.set_align(FormatAlign::CenterAcross),
        HAlign::Distributed => format.set_align(FormatAlign::Distributed),
    };
    format = match style.v_align {
        VAlign::Top => format.set_align(FormatAlign::Top),
        VAlign::Center => format.set_align(FormatAlign::VerticalCenter),
        VAlign::Bottom => format,
        VAlign::Justify => format.set_align(FormatAlign::VerticalJustify),
        VAlign::Distributed => format.set_align(FormatAlign::VerticalDistributed),
    };
    if style.wrap {
        format = format.set_text_wrap();
    }
    if style.shrink {
        format = format.set_shrink();
    }
    if style.indent > 0 {
        format = format.set_indent(style.indent);
    }
    if style.is_stacked() {
        format = format.set_rotation(270);
    } else if style.rotation != 0 {
        format = format.set_rotation(style.rotation);
    }
    if !style.locked {
        format = format.set_unlocked();
    }
    if style.hidden {
        format = format.set_hidden();
    }
    format
}

fn xlsx_pattern(kind: PatternKind) -> FormatPattern {
    match kind {
        PatternKind::Gray75 => FormatPattern::DarkGray,
        PatternKind::Gray50 => FormatPattern::MediumGray,
        PatternKind::Gray25 => FormatPattern::LightGray,
        PatternKind::Gray125 => FormatPattern::Gray125,
        PatternKind::Gray0625 => FormatPattern::Gray0625,
        PatternKind::DarkHorizontal => FormatPattern::DarkHorizontal,
        PatternKind::DarkVertical => FormatPattern::DarkVertical,
        PatternKind::DarkDown => FormatPattern::DarkDown,
        PatternKind::DarkUp => FormatPattern::DarkUp,
        PatternKind::DarkGrid => FormatPattern::DarkGrid,
        PatternKind::DarkTrellis => FormatPattern::DarkTrellis,
        PatternKind::LightHorizontal => FormatPattern::LightHorizontal,
        PatternKind::LightVertical => FormatPattern::LightVertical,
        PatternKind::LightDown => FormatPattern::LightDown,
        PatternKind::LightUp => FormatPattern::LightUp,
        PatternKind::LightGrid => FormatPattern::LightGrid,
        PatternKind::LightTrellis => FormatPattern::LightTrellis,
    }
}

fn at(top: u32, left: u32, row: usize, col: usize) -> Result<CellAddr, Error> {
    let offset = |base: u32, n: usize| base.saturating_add(u32::try_from(n).unwrap_or(u32::MAX));
    Ok(CellAddr::new(offset(top, row), offset(left, col))?)
}

fn canonical_formula(text: &str) -> Option<String> {
    let body = match text
        .strip_prefix("of:")
        .or_else(|| text.strip_prefix("oooc:"))
    {
        Some(open) => openformula::to_canonical(open.strip_prefix('=').unwrap_or(open))?,
        None => text
            .strip_prefix('=')
            .unwrap_or(text)
            .replace("_xlfn.", "")
            .replace("_xlws.", ""),
    };
    let expr = formula::parse(&body, &EN).ok()?;
    Some(format!("={}", expr.write(&EN)))
}

fn convert(data: &Data) -> Option<Cell> {
    let (value, code) = match data {
        Data::Int(i) => (CellValue::Number(*i as f64), None),
        Data::Float(f) => (CellValue::Number(*f), None),
        Data::String(s) => (CellValue::Text(s.clone()), None),
        Data::Bool(b) => (CellValue::Bool(*b), None),
        Data::DateTime(dt) if dt.is_duration() => {
            (CellValue::Number(dt.as_f64()), Some(numfmt::TIME_SECONDS))
        }
        Data::DateTime(dt) => (CellValue::Number(dt.as_f64()), Some(temporal(dt.as_f64()))),
        Data::DateTimeIso(s) => {
            let serial = iso_date_time(s)?;
            (CellValue::Number(serial), Some(temporal(serial)))
        }
        Data::DurationIso(s) => (
            CellValue::Number(iso_duration(s)?),
            Some(numfmt::TIME_SECONDS),
        ),
        Data::Error(e) => (CellValue::Error(error(e)), None),
        Data::Empty => return None,
    };
    let mut cell = Cell::from(value);
    if let Some(code) = code {
        cell.set_style(Style::with_number_format(code));
    }
    Some(cell)
}

fn temporal(serial: f64) -> &'static str {
    if serial < 1.0 {
        numfmt::TIME
    } else if serial.fract() == 0.0 {
        numfmt::DATE
    } else {
        numfmt::DATE_TIME
    }
}

fn iso_date_time(text: &str) -> Option<f64> {
    let text = text.split('.').next()?.replace('T', " ");
    EN.parse_date_time(&text).map(|(serial, _)| serial)
}

fn iso_duration(text: &str) -> Option<f64> {
    let mut seconds = 0.0;
    let mut number = String::new();
    for c in text.strip_prefix("PT")?.chars() {
        let unit = match c {
            'H' => 3600.0,
            'M' => 60.0,
            'S' => 1.0,
            c => {
                number.push(c);
                continue;
            }
        };
        seconds += number.parse::<f64>().ok()? * unit;
        number.clear();
    }
    Some(seconds / SECONDS_PER_DAY)
}

fn error(e: &CellErrorType) -> CellError {
    match e {
        CellErrorType::Div0 => CellError::Div0,
        CellErrorType::Name => CellError::Name,
        CellErrorType::Null => CellError::Null,
        CellErrorType::Num => CellError::Num,
        CellErrorType::Ref => CellError::Ref,
        CellErrorType::Value => CellError::Value,
        CellErrorType::NA | CellErrorType::GettingData => CellError::NA,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vellum_grid_core::Document;

    fn addr(s: &str) -> CellAddr {
        s.parse().unwrap()
    }

    fn sample() -> Workbook {
        let mut summary = Sheet::new("Özet");
        let mut data = Sheet::new("Veri Tablosu");
        for (a, text) in [
            ("A1", "Toplam"),
            ("B1", "=SUM('Veri Tablosu'!B:B)"),
            ("A2", "Tarih"),
            ("B2", "2026-10-17"),
            ("B3", "=B2+7"),
            (
                "B4",
                "=XLOOKUP(\"elma\",'Veri Tablosu'!A:A,'Veri Tablosu'!B:B)",
            ),
            ("B5", "TRUE"),
            ("B6", "#N/A"),
            ("B7", "13:45:00"),
        ] {
            summary.set_input(addr(a), text);
        }
        for (a, text) in [("A1", "elma"), ("B1", "2.5"), ("A2", "armut"), ("B2", "4")] {
            data.set_input(addr(a), text);
        }
        let book = Workbook::from_sheets(vec![summary, data]).unwrap();
        Document::new(book).book().clone()
    }

    #[test]
    fn xlsx_round_trip_keeps_pictures_and_shapes() {
        use vellum_grid_core::drawing::{Picture, Shape, ShapeKind};
        const PNG: [u8; 70] = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78,
            0xDA, 0x63, 0x64, 0x60, 0xF8, 0x5F, 0x0F, 0x00, 0x02, 0x87, 0x01, 0x80, 0xEB, 0x47,
            0xBA, 0x92, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let book = sample();
        let mut first = book.sheets()[0].clone();
        let mut second = book.sheets()[1].clone();
        let mut picture = Picture::new(PNG.to_vec(), addr("C2"), (40, 30)).unwrap();
        picture.offset = (5, 3);
        let mut other = picture.clone();
        other.size = (12, 12);
        first.layout_mut().pictures = vec![picture, other];
        let mut note = Shape::new(ShapeKind::TextBox, addr("E4"));
        note.text = "Not".to_owned();
        second.layout_mut().shapes = vec![Shape::new(ShapeKind::RightArrow, addr("B2")), note];
        let book = Workbook::from_sheets(vec![first, second]).unwrap();
        let read = read_workbook(write_xlsx(&book).unwrap()).unwrap();
        for index in 0..2 {
            let (a, b) = (book.sheets()[index].layout(), read.sheets()[index].layout());
            assert_eq!(a.pictures, b.pictures);
            assert_eq!(a.shapes, b.shapes);
        }
    }

    #[test]
    fn xlsx_round_trip_keeps_hidden_sheets() {
        let book = sample();
        let mut hidden = book.sheets()[1].clone();
        hidden.layout_mut().sheet_hidden = true;
        let book = Workbook::from_sheets(vec![book.sheets()[0].clone(), hidden]).unwrap();
        let read = read_workbook(write_xlsx(&book).unwrap()).unwrap();
        assert!(!read.sheets()[0].layout().sheet_hidden);
        assert!(read.sheets()[1].layout().sheet_hidden);
    }

    #[test]
    fn xlsx_round_trip_keeps_sheets_values_formulas_and_dates() {
        let bytes = write_xlsx(&sample()).unwrap();
        let book = read_workbook(bytes).unwrap();
        let names: Vec<&str> = book.sheets().iter().map(Sheet::name).collect();
        assert_eq!(names, ["Özet", "Veri Tablosu"]);

        let summary = &book.sheets()[0];
        let cell = |a: &str| summary.get(addr(a)).unwrap();
        assert_eq!(cell("A1").value, CellValue::Text("Toplam".into()));
        assert_eq!(
            cell("B1").formula.as_deref(),
            Some("=SUM('Veri Tablosu'!B:B)")
        );
        assert_eq!(cell("B1").value, CellValue::Number(6.5));
        assert_eq!(cell("B2").value, CellValue::Number(46312.0));
        assert_eq!(cell("B2").number_format(), numfmt::DATE);
        assert_eq!(cell("B3").number_format(), numfmt::DATE);
        assert_eq!(
            cell("B4").formula.as_deref(),
            Some("=XLOOKUP(\"elma\",'Veri Tablosu'!A:A,'Veri Tablosu'!B:B)")
        );
        assert_eq!(cell("B5").value, CellValue::Bool(true));
        assert_eq!(cell("B6").value, CellValue::Error(CellError::NA));
        assert_eq!(cell("B7").number_format(), numfmt::TIME_SECONDS);

        let doc = Document::new(book);
        assert_eq!(doc.sheet().value(addr("B3")), &CellValue::Number(46319.0));
        assert_eq!(doc.sheet().value(addr("B4")), &CellValue::Number(2.5));
    }

    #[test]
    fn xlsx_round_trip_keeps_formatting_and_layout() {
        let mut sheet = Sheet::new("Biçim");
        sheet.set_input(addr("A1"), "Başlık");
        sheet.set_input(addr("B2"), "1234.5");
        let mut doc = Document::new(Workbook::from_sheets(vec![sheet]).unwrap());
        doc.style_cells("A1".parse().unwrap(), |_, s| {
            s.bold = true;
            s.italic = true;
            s.size = 160;
            s.color = Some(vellum_grid_core::Color(0xC00000));
            s.fill = Some(vellum_grid_core::Color(0xFFFF00));
            s.h_align = HAlign::Center;
            s.v_align = VAlign::Center;
            s.wrap = true;
        });
        doc.style_cells("B2".parse().unwrap(), |_, s| {
            s.number_format = numfmt::CURRENCY.into();
            s.borders.bottom = Some(vellum_grid_core::Border {
                style: LineStyle::Double,
                color: vellum_grid_core::Color(0x0000FF),
            });
        });
        doc.merge("A1:C1".parse().unwrap(), false);
        doc.set_layout(|l| {
            l.col_widths.insert(1, 20.0);
            l.row_heights.insert(0, 30.0);
            l.hide_gridlines = true;
        });
        let bytes = write_xlsx(doc.book()).unwrap();
        let book = read_workbook(bytes).unwrap();
        let sheet = &book.sheets()[0];
        let a1 = sheet.get(addr("A1")).unwrap().style();
        assert!(a1.bold && a1.italic && a1.wrap);
        assert_eq!(a1.size, 160);
        assert_eq!(a1.color, Some(vellum_grid_core::Color(0xC00000)));
        assert_eq!(a1.fill, Some(vellum_grid_core::Color(0xFFFF00)));
        assert_eq!((a1.h_align, a1.v_align), (HAlign::Center, VAlign::Center));
        let b2 = sheet.get(addr("B2")).unwrap();
        assert_eq!(b2.number_format(), numfmt::CURRENCY);
        assert_eq!(
            b2.style().borders.bottom.map(|b| b.style),
            Some(LineStyle::Double)
        );
        assert_eq!(b2.value, CellValue::Number(1234.5));
        let layout = sheet.layout();
        assert_eq!(layout.merges, ["A1:C1".parse().unwrap()]);
        assert!((layout.col_width(1) - 20.0).abs() < 0.2);
        assert_eq!(layout.row_height(0), 30.0);
        assert!(layout.hide_gridlines);
    }

    #[test]
    fn xlsx_round_trip_keeps_view_filters_rules_and_charts() {
        use vellum_grid_core::{AutoFilter, Chart, ConditionalFormat, Highlight};
        let mut sheet = Sheet::new("Özet Sayfa");
        for (a, text) in [
            ("A1", "Ay"),
            ("B1", "Gelir"),
            ("A2", "Ocak"),
            ("B2", "10"),
            ("A3", "Şubat"),
            ("B3", "20"),
            ("A4", "Mart"),
            ("B4", "30"),
        ] {
            sheet.set_input(addr(a), text);
        }
        let red = Highlight {
            fill: Some(vellum_grid_core::Color(0xFFC7CE)),
            color: Some(vellum_grid_core::Color(0x9C0006)),
            bold: true,
            italic: true,
            strike: false,
            border: Some(vellum_grid_core::Border::thin()),
        };
        let layout = sheet.layout_mut();
        layout.freeze_rows = 1;
        layout.freeze_cols = 1;
        layout.tab_color = Some(vellum_grid_core::Color(0x00B050));
        layout.hidden_cols.insert(5);
        layout.hidden_rows.insert(2);
        let criteria = [
            (
                0,
                ColumnFilter::Values(["Ocak".to_owned(), "Mart".to_owned()].into_iter().collect()),
            ),
            (
                1,
                ColumnFilter::Custom {
                    first: FilterRule {
                        op: Operator::Greater,
                        value: "1.5".into(),
                    },
                    second: Some(FilterRule {
                        op: Operator::Contains,
                        value: "ar".into(),
                    }),
                    and: false,
                },
            ),
        ];
        layout.filter = Some(AutoFilter {
            range: "A1:B4".parse().unwrap(),
            criteria: criteria.into_iter().collect(),
        });
        let mut bar = vellum_grid_core::DataBar::new(vellum_grid_core::Color(0x638EC6), false);
        bar.max = vellum_grid_core::Threshold::Number(50.0);
        let scale = vellum_grid_core::ColorScale {
            points: vec![
                (
                    vellum_grid_core::Threshold::Lowest,
                    vellum_grid_core::Color(0xF8696B),
                ),
                (
                    vellum_grid_core::Threshold::Percentile(50.0),
                    vellum_grid_core::Color(0xFFEB84),
                ),
                (
                    vellum_grid_core::Threshold::Highest,
                    vellum_grid_core::Color(0x63BE7B),
                ),
            ],
        };
        let mut icons = vellum_grid_core::IconSet::new(IconKind::Rating4);
        icons.reverse = true;
        icons.show_value = false;
        let mut custom = vellum_grid_core::IconSet::new(IconKind::Arrows3);
        custom.thresholds = vec![
            vellum_grid_core::Threshold::Number(10.0),
            vellum_grid_core::Threshold::Number(25.0),
        ];
        for (index, rule) in [
            Rule::Greater(15.0),
            Rule::Between(1.0, 5.0),
            Rule::Equal(CellValue::Text("Ocak".into())),
            Rule::TextContains("ar".into()),
            Rule::Duplicate,
            Rule::NotBetween(2.0, 4.0),
            Rule::LessEqual(9.0),
            Rule::NotEqual(CellValue::Number(3.0)),
            Rule::TextNotContains("x".into()),
            Rule::BeginsWith("O".into()),
            Rule::EndsWith("k".into()),
            Rule::Unique,
            Rule::Blanks,
            Rule::NoErrors,
            Rule::Dates(DatePeriod::LastMonth),
            Rule::Top {
                count: 15,
                percent: true,
                bottom: true,
            },
            Rule::Average {
                below: true,
                equal: false,
                deviations: 2,
            },
            Rule::Expression("MOD(B2,2)=0".into()),
            Rule::DataBar(bar),
            Rule::ColorScale(scale),
            Rule::IconSet(icons),
            Rule::IconSet(custom),
        ]
        .into_iter()
        .enumerate()
        {
            let format = if rule.is_graphic() {
                Highlight::default()
            } else {
                red
            };
            let mut conditional = ConditionalFormat::new("B2:B4".parse().unwrap(), rule, format);
            conditional.stop = index == 1;
            layout.conditional.push(conditional);
        }
        let mut bar = Chart::new(ChartKind::Bar, "A1:B4".parse().unwrap(), addr("D2"));
        bar.title = Some("Gelirler".into());
        bar.offset = (5, 3);
        bar.size = (400, 250);
        bar.legend = Legend::Hidden;
        layout.charts.push(bar);
        let mut area = Chart::new(ChartKind::Area, "A1:B4".parse().unwrap(), addr("D20"));
        area.grouping = Grouping::Percent;
        area.legend = Legend::Left;
        area.labels = true;
        area.x_title = Some("Ay".into());
        area.y_title = Some("Tutar".into());
        area.gridlines = false;
        layout.charts.push(area);
        let mut line = Chart::new(ChartKind::Line, "A1:B4".parse().unwrap(), addr("D40"));
        line.grouping = Grouping::Stacked;
        line.markers = true;
        line.legend = Legend::Top;
        layout.charts.push(line);
        let mut ring = Chart::new(ChartKind::Doughnut, "A1:B4".parse().unwrap(), addr("D60"));
        ring.legend = Legend::Right;
        ring.gridlines = false;
        layout.charts.push(ring);
        let mut group = vellum_grid_core::SparklineGroup::new(
            SparkKind::Column,
            vellum_grid_core::spark_pairs("B2:B4".parse().unwrap(), "C2:C4".parse().unwrap())
                .unwrap()
                .into_iter()
                .take(1)
                .collect(),
        );
        group.high = true;
        group.color = vellum_grid_core::Color(0x00B050);
        layout.sparklines.push(group);
        let mut pair = vellum_grid_core::SparklineGroup::new(
            SparkKind::Line,
            vec![
                (addr("F2"), "A2:B2".parse().unwrap()),
                (addr("F3"), "A3:B3".parse().unwrap()),
            ],
        );
        pair.markers = true;
        layout.sparklines.push(pair);
        let expected = sheet.layout().clone();
        let bytes = write_xlsx(&Workbook::from_sheets(vec![sheet]).unwrap()).unwrap();
        let book = read_workbook(bytes).unwrap();
        let layout = book.sheets()[0].layout();
        assert_eq!((layout.freeze_rows, layout.freeze_cols), (1, 1));
        assert_eq!(layout.tab_color, expected.tab_color);
        assert_eq!(layout.hidden_cols, expected.hidden_cols);
        assert_eq!(layout.hidden_rows, expected.hidden_rows);
        assert_eq!(layout.filter, expected.filter);
        assert_eq!(layout.conditional, expected.conditional);
        assert_eq!(layout.charts.len(), 4);
        for (read, wanted) in layout.charts.iter().zip(&expected.charts).skip(1) {
            let normalized = Chart {
                anchor: read.anchor,
                offset: read.offset,
                size: read.size,
                ..wanted.clone()
            };
            assert_eq!(read, &normalized);
        }
        assert_eq!(layout.sparklines, expected.sparklines);
        let chart = &layout.charts[0];
        assert_eq!(chart.kind, ChartKind::Bar);
        assert_eq!(chart.data, "A1:B4".parse().unwrap());
        assert_eq!(chart.title.as_deref(), Some("Gelirler"));
        assert_eq!(chart.anchor, addr("D2"));
        assert_eq!(chart.offset, (5, 3));
        assert!(chart.size.0.abs_diff(400) <= 2 && chart.size.1.abs_diff(250) <= 2);
    }

    #[test]
    fn reads_ods() {
        let bytes = include_bytes!("../tests/data/sample.ods").to_vec();
        let book = read_workbook(bytes).unwrap();
        let names: Vec<&str> = book.sheets().iter().map(Sheet::name).collect();
        assert_eq!(names, ["Gelir", "Gider"]);
        let gelir = &book.sheets()[0];
        let cell = |a: &str| gelir.get(addr(a)).unwrap();
        assert_eq!(cell("A1").value, CellValue::Text("Kalem".into()));
        assert_eq!(cell("B2").value, CellValue::Number(1250.5));
        assert_eq!(cell("B4").formula.as_deref(), Some("=SUM(B2:B3)+Gider!B2"));
        assert_eq!(cell("C2").value, CellValue::Number(46312.0));
        assert_eq!(cell("C2").number_format(), numfmt::DATE);
        assert_eq!(cell("D2").value, CellValue::Bool(true));

        let doc = Document::new(book);
        assert_eq!(doc.sheet().value(addr("B4")), &CellValue::Number(1350.5));
    }

    #[test]
    fn page_setup_round_trips() {
        let mut sheet = Sheet::new("Rapor");
        sheet.set_input(addr("A1"), "x");
        sheet.set_input(addr("D40"), "y");
        let page = &mut sheet.layout_mut().page;
        page.paper = vellum_grid_core::Paper::A3;
        page.landscape = true;
        page.margins = vellum_grid_core::Margins::NARROW;
        page.scale = Scale::Fit {
            width: 1,
            height: 0,
        };
        page.print_area = Some("A1:D30".parse().unwrap());
        page.title_rows = Some((0, 1));
        page.title_cols = Some((0, 0));
        page.header = "&CRapor".to_owned();
        page.footer = "&RSayfa &P / &N".to_owned();
        page.gridlines = true;
        page.center_horizontally = true;
        page.row_breaks.insert(20);
        page.col_breaks.insert(2);
        let expected = page.clone();
        let book = Workbook::from_sheets(vec![sheet, Sheet::new("İkinci")]).unwrap();
        let back = read_workbook(write_xlsx(&book).unwrap()).unwrap();
        assert_eq!(back.sheets()[0].layout().page, expected);
        assert_eq!(back.sheets()[1].layout().page, PageSetup::default());
    }

    #[test]
    fn tables_round_trip() {
        let mut doc = Document::with_locale(
            Workbook::from_sheets(vec![Sheet::new("Veri")]).unwrap(),
            &vellum_grid_core::TR,
        );
        for (cell, input) in [
            ("A1", "Ürün"),
            ("B1", "Adet"),
            ("A2", "Kalem"),
            ("B2", "3"),
            ("A3", "Defter"),
            ("B3", "5"),
            ("C1", "Çift"),
            ("C2", "=[@Adet]*2"),
            ("C3", "=[@Adet]*2"),
            ("E1", "=TOPLA(Tablo1[Adet])"),
        ] {
            doc.enter(addr(cell), input).unwrap();
        }
        doc.create_table(CellRange::new(addr("A1"), addr("C3")), true)
            .unwrap();
        doc.update_table(0, |t| {
            t.totals = true;
            t.style = "TableStyleLight9".into();
            t.banded_cols = true;
        });
        let expected = doc.sheet().layout().tables.clone();
        let back = read_workbook(write_xlsx(doc.book()).unwrap()).unwrap();
        let read = &back.sheets()[0].layout().tables;
        assert_eq!(read, &expected);
        let cell = back.sheets()[0].get(addr("C2")).unwrap();
        assert_eq!(
            cell.formula.as_deref(),
            Some("=Tablo1[[#This Row],[Adet]]*2")
        );
        let reopened = Document::new(back);
        assert_eq!(reopened.sheet().value(addr("E1")), &CellValue::Number(8.0));
        assert_eq!(reopened.sheet().value(addr("C4")), &CellValue::Number(16.0));
    }

    #[test]
    fn extended_styles_round_trip() {
        use vellum_grid_core::{Border, Borders, Color, Pattern, STACKED};
        let styles = [
            Style {
                h_align: HAlign::CenterAcross,
                v_align: VAlign::Justify,
                indent: 0,
                rotation: 45,
                shrink: true,
                underline: Underline::Double,
                script: Script::Super,
                fill: Some(Color(0xFFFF00)),
                pattern: Some(Pattern {
                    kind: PatternKind::LightTrellis,
                    color: Color(0x0000FF),
                }),
                borders: Borders {
                    top: Some(Border {
                        style: LineStyle::Hair,
                        color: Color(0xFF0000),
                    }),
                    diagonal: Some(Border {
                        style: LineStyle::SlantDashDot,
                        color: Color::BLACK,
                    }),
                    diagonal_up: true,
                    diagonal_down: true,
                    ..Borders::default()
                },
                ..Style::default()
            },
            Style {
                h_align: HAlign::Right,
                indent: 3,
                rotation: -30,
                underline: Underline::DoubleAccounting,
                script: Script::Sub,
                ..Style::default()
            },
            Style {
                rotation: STACKED,
                h_align: HAlign::Distributed,
                v_align: VAlign::Distributed,
                ..Style::default()
            },
        ];
        let mut sheet = Sheet::new("Biçim");
        for (index, style) in styles.iter().enumerate() {
            let at = CellAddr::new(index as u32, 0).unwrap();
            sheet.set_input(at, "Metin");
            let mut cell = sheet.get(at).cloned().unwrap();
            cell.set_style(style.clone());
            sheet.set(at, cell);
        }
        let book = Workbook::from_sheets(vec![sheet]).unwrap();
        let back = read_workbook(write_xlsx(&book).unwrap()).unwrap();
        for (index, style) in styles.iter().enumerate() {
            let at = CellAddr::new(index as u32, 0).unwrap();
            assert_eq!(
                back.sheets()[0].get(at).unwrap().style(),
                style,
                "row {index}"
            );
        }
    }

    #[test]
    fn review_and_protection_round_trip() {
        use vellum_grid_core::protect::{PasswordHash, Permissions, Protection};
        use vellum_grid_core::review::{Comment, Link, Note, Reply};
        let mut sheet = Sheet::new("Özet");
        sheet.set_input(addr("A1"), "Bütçe");
        sheet.set_input(addr("B2"), "Siteye git");
        let unlocked = Style {
            locked: false,
            hidden: true,
            ..Style::default()
        };
        let mut cell = sheet.get(addr("A1")).cloned().unwrap();
        cell.set_style(unlocked.clone());
        sheet.set(addr("A1"), cell);
        let layout = sheet.layout_mut();
        layout.notes.insert(
            addr("A1"),
            Note {
                author: "Ayşe".into(),
                text: "Ocak rakamları & kesin değil".into(),
                visible: false,
            },
        );
        layout.comments.insert(
            addr("C3"),
            Comment {
                thread: vec![
                    Reply {
                        author: "Ali".into(),
                        text: "KDV dahil mi?".into(),
                        time: 45366.5,
                    },
                    Reply {
                        author: "Ayşe".into(),
                        text: "Evet <dahil>".into(),
                        time: 45367.25,
                    },
                ],
                resolved: true,
            },
        );
        layout.links.insert(
            addr("B2"),
            Link {
                target: "https://ornek.com.tr/".into(),
                tooltip: "Ana sayfa".into(),
            },
        );
        layout.links.insert(
            addr("B3"),
            Link {
                target: "#'Özet'!A1".into(),
                tooltip: String::new(),
            },
        );
        layout.protection = Some(Protection {
            password: PasswordHash::of("gizli"),
            allow: Permissions {
                format_columns: true,
                sort: true,
                ..Permissions::default()
            },
        });
        let expected = layout.clone();
        let mut book = Workbook::from_sheets(vec![sheet]).unwrap();
        book.protection = Some(PasswordHash::of("kitap"));
        let back = read_workbook(write_xlsx(&book).unwrap()).unwrap();
        let read = back.sheets()[0].layout();
        assert_eq!(read.notes, expected.notes);
        assert_eq!(read.comments, expected.comments);
        assert_eq!(read.links, expected.links);
        assert_eq!(read.protection, expected.protection);
        assert_eq!(back.protection, book.protection);
        let style = back.sheets()[0].get(addr("A1")).unwrap().style().clone();
        assert!(!style.locked);
        assert!(style.hidden);
    }

    #[test]
    fn outline_round_trips() {
        let mut sheet = Sheet::new("S");
        for row in 0..8 {
            sheet.set_input(CellAddr::new(row, 0).unwrap(), "1");
        }
        let layout = sheet.layout_mut();
        outline::group(&mut layout.row_levels, 1, 5);
        outline::group(&mut layout.row_levels, 2, 3);
        outline::collapse(
            &mut layout.hidden_rows,
            &mut layout.collapsed_rows,
            outline::Group {
                level: 2,
                start: 2,
                end: 3,
            },
        );
        outline::group(&mut layout.col_levels, 2, 3);
        let expected = layout.clone();
        let book = Workbook::from_sheets(vec![sheet]).unwrap();
        let back = read_workbook(write_xlsx(&book).unwrap()).unwrap();
        let read = back.sheets()[0].layout();
        assert_eq!(read.row_levels, expected.row_levels);
        assert_eq!(read.col_levels, expected.col_levels);
        assert_eq!(read.collapsed_rows, expected.collapsed_rows);
        assert_eq!(read.hidden_rows, expected.hidden_rows);
    }

    #[test]
    fn validations_round_trip() {
        let mut sheet = Sheet::new("S");
        sheet.layout_mut().validations = vec![
            Validation {
                allow: Allow::List,
                formula1: "\"Evet,Hayır\"".into(),
                input_title: "Seçin".into(),
                input_message: "Listeden seçin".into(),
                style: AlertStyle::Warning,
                ..Validation::new(vec!["A1:A5".parse().unwrap(), "C1".parse().unwrap()])
            },
            Validation {
                allow: Allow::Whole,
                compare: Compare::Between,
                formula1: "1".into(),
                formula2: "$F$1".into(),
                dropdown: false,
                error_message: "1 ile F1 arasında olmalı".into(),
                ..Validation::new(vec!["B1:B9".parse().unwrap()])
            },
            Validation {
                allow: Allow::List,
                formula1: "$E$1:$E$3".into(),
                ignore_blank: false,
                ..Validation::new(vec!["D2".parse().unwrap()])
            },
            Validation {
                allow: Allow::TextLength,
                compare: Compare::LessOrEqual,
                formula1: "10".into(),
                ..Validation::new(vec!["G1".parse().unwrap()])
            },
        ];
        let expected = sheet.layout().validations.clone();
        let book = Workbook::from_sheets(vec![sheet]).unwrap();
        let back = read_workbook(write_xlsx(&book).unwrap()).unwrap();
        assert_eq!(back.sheets()[0].layout().validations, expected);
    }

    #[test]
    fn defined_names_round_trip() {
        let mut sheet = Sheet::new("Satış Verisi");
        sheet.set_input(addr("A1"), "4");
        sheet.set_input(addr("B1"), "=Fiyat*Oran");
        let mut book = Workbook::from_sheets(vec![sheet, Sheet::new("İkinci")]).unwrap();
        book.add_name(DefinedName {
            comment: "birim fiyat".into(),
            ..DefinedName::new("Fiyat", "'Satış Verisi'!$A$1")
        })
        .unwrap();
        book.add_name(DefinedName::new("Oran", "0.5")).unwrap();
        book.set_manual_calculation(true);
        book.add_name(DefinedName {
            scope: Some(1),
            ..DefinedName::new("Oran", "2")
        })
        .unwrap();
        let back = read_workbook(write_xlsx(&book).unwrap()).unwrap();
        assert!(back.manual_calculation());
        let mut names: Vec<_> = back
            .names()
            .iter()
            .map(|n| (n.name.as_str(), n.scope, n.formula.as_str()))
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                ("Fiyat", None, "'Satış Verisi'!$A$1"),
                ("Oran", None, "0.5"),
                ("Oran", Some(1), "2"),
            ]
        );
        assert_eq!(
            back.sheets()[0].get(addr("B1")).unwrap().formula.as_deref(),
            Some("=Fiyat*Oran")
        );
    }

    #[test]
    fn newer_functions_get_the_future_prefix() {
        let mut sheet = Sheet::new("S");
        sheet.set_input(addr("A1"), "=IFS(TRUE,1)+DAYS(2,1)");
        sheet.set_input(addr("A2"), "=CONCAT(\"a\",TEXTJOIN(\",\",TRUE,\"b\"))");
        sheet.set_input(addr("A3"), "=SUM(1)");
        let bytes = write_xlsx(&Workbook::from_sheets(vec![sheet]).unwrap()).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut xml = String::new();
        std::io::Read::read_to_string(
            &mut archive.by_name("xl/worksheets/sheet1.xml").unwrap(),
            &mut xml,
        )
        .unwrap();
        assert!(xml.contains("_xlfn.IFS(TRUE,1)+_xlfn.DAYS(2,1)"), "{xml}");
        assert!(xml.contains("_xlfn.CONCAT(\"a\",_xlfn.TEXTJOIN("), "{xml}");
        assert!(!xml.contains("_xlfn._xlfn"));
        assert!(!xml.contains("_xlfn.SUM"));
    }

    #[test]
    fn converts_iso_values() {
        assert_eq!(iso_date_time("2026-10-17T12:00:00.000"), Some(46312.5));
        assert_eq!(iso_duration("PT13H45M00S"), Some(13.75 / 24.0));
        assert_eq!(iso_duration("13:45"), None);
    }

    #[test]
    fn keeps_unparseable_formulas_out() {
        assert_eq!(canonical_formula("SUM(A1"), Some("=SUM(A1)".into()));
        assert_eq!(
            canonical_formula("_xlfn.CONCAT(A1,B1)"),
            Some("=CONCAT(A1,B1)".into())
        );
        assert_eq!(canonical_formula("A1 B1"), None);
    }
}
