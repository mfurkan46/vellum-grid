use eframe::egui::{self, Area, ComboBox, Context, Id, Order, Rect, RichText, TextEdit, Ui, vec2};
use vellum_grid_core::review::{Link, LinkKind, normalize_target};
use vellum_grid_core::{CellAddr, CellValue};

use crate::app::{Spreadsheet, locale};
use crate::dialogs::Dialog;
use crate::{text, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkTab {
    Address,
    Place,
    Mail,
}

#[derive(Debug, Clone)]
pub struct LinkForm {
    pub addr: CellAddr,
    pub tab: LinkTab,
    pub display: String,
    pub address: String,
    pub sheet: usize,
    pub reference: String,
    pub email: String,
    pub subject: String,
    pub tooltip: String,
    pub existing: bool,
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                char::from(b).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let Some(byte) = text
                .get(index + 1..index + 3)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn quoted_sheet(name: &str) -> String {
    format!("'{}'", name.replace('\'', "''"))
}

pub fn target(form: &LinkForm, sheets: &[String]) -> Option<String> {
    match form.tab {
        LinkTab::Address => {
            let address = form.address.trim();
            (!address.is_empty()).then(|| normalize_target(address))
        }
        LinkTab::Place => {
            let reference = form.reference.trim();
            let sheet = sheets.get(form.sheet)?;
            let reference = if reference.is_empty() {
                "A1"
            } else {
                reference
            };
            Some(format!("#{}!{}", quoted_sheet(sheet), reference))
        }
        LinkTab::Mail => {
            let email = form.email.trim();
            if email.is_empty() {
                return None;
            }
            let subject = form.subject.trim();
            Some(if subject.is_empty() {
                format!("mailto:{email}")
            } else {
                format!("mailto:{email}?subject={}", encode(subject))
            })
        }
    }
}

pub fn form_ui(ui: &mut Ui, form: &mut LinkForm, sheets: &[String]) -> bool {
    ui.set_width(theme::LINK_DIALOG_WIDTH);
    ui.horizontal(|ui| {
        for (tab, label) in [
            (LinkTab::Address, text::link_file_web()),
            (LinkTab::Place, text::link_place()),
            (LinkTab::Mail, text::link_mail()),
        ] {
            ui.selectable_value(&mut form.tab, tab, label);
        }
    });
    ui.separator();
    let field = |ui: &mut Ui, label: &str, value: &mut String| {
        ui.label(label);
        ui.add(
            TextEdit::singleline(value)
                .desired_width(f32::INFINITY)
                .background_color(theme::input_bg()),
        );
    };
    field(ui, text::display_text(), &mut form.display);
    match form.tab {
        LinkTab::Address => field(ui, text::link_address(), &mut form.address),
        LinkTab::Place => {
            field(ui, text::link_reference(), &mut form.reference);
            ui.label(text::link_sheet());
            ComboBox::from_id_salt("link_sheet")
                .selected_text(sheets.get(form.sheet).map_or("", String::as_str))
                .width(theme::NAME_INPUT_WIDTH)
                .show_ui(ui, |ui| {
                    for (index, sheet) in sheets.iter().enumerate() {
                        ui.selectable_value(&mut form.sheet, index, sheet);
                    }
                });
        }
        LinkTab::Mail => {
            field(ui, text::link_email(), &mut form.email);
            field(ui, text::link_subject(), &mut form.subject);
        }
    }
    field(ui, text::screen_tip(), &mut form.tooltip);
    form.existing
        && ui
            .add(crate::dialogs::plain_button(text::remove_link()))
            .clicked()
}

pub fn open_external(target: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(target).spawn();
    #[cfg(all(unix, not(target_arch = "wasm32")))]
    let _ = std::process::Command::new("xdg-open").arg(target).spawn();
    #[cfg(target_arch = "wasm32")]
    {
        use eframe::wasm_bindgen::JsCast;
        if let Some(anchor) = eframe::web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.create_element("a").ok())
            .and_then(|e| e.dyn_into::<eframe::web_sys::HtmlAnchorElement>().ok())
        {
            anchor.set_href(target);
            anchor.set_target("_blank");
            anchor.click();
        }
    }
}

impl Spreadsheet {
    pub fn open_link_dialog(&mut self) {
        let addr = self.sel.active;
        let existing = self.sheet().layout().links.get(&addr).cloned();
        let display = match self.sheet().value(addr) {
            CellValue::Empty => String::new(),
            value => locale().display(value),
        };
        let mut form = LinkForm {
            addr,
            tab: LinkTab::Address,
            display,
            address: String::new(),
            sheet: self.doc.active(),
            reference: "A1".to_owned(),
            email: String::new(),
            subject: String::new(),
            tooltip: String::new(),
            existing: existing.is_some(),
        };
        if let Some(link) = existing {
            form.tooltip = link.tooltip.clone();
            match link.kind() {
                LinkKind::Place => {
                    form.tab = LinkTab::Place;
                    let place = link.place().unwrap_or_default();
                    if let Some((sheet, reference)) = place.rsplit_once('!') {
                        let name = sheet.trim_matches('\'').replace("''", "'");
                        form.sheet = self.doc.book().index_of(&name).unwrap_or(form.sheet);
                        form.reference = reference.to_owned();
                    } else {
                        form.reference = place.to_owned();
                    }
                }
                LinkKind::Mail => {
                    form.tab = LinkTab::Mail;
                    let body = &link.target["mailto:".len()..];
                    let (email, query) = body.split_once('?').unwrap_or((body, ""));
                    form.email = email.to_owned();
                    form.subject = query
                        .strip_prefix("subject=")
                        .map(decode)
                        .unwrap_or_default();
                }
                LinkKind::Web | LinkKind::File => form.address = link.target.clone(),
            }
        }
        self.dialog = Some(Dialog::Link(form));
    }

    pub fn apply_link(&mut self, form: LinkForm) {
        let Some(target) = target(&form, &self.sheet_names()) else {
            self.dialog = Some(Dialog::Link(form));
            return;
        };
        let display = if form.display.trim().is_empty() {
            target
                .trim_start_matches('#')
                .trim_start_matches("mailto:")
                .to_owned()
        } else {
            form.display.clone()
        };
        let link = Link {
            target,
            tooltip: form.tooltip.trim().to_owned(),
        };
        self.doc.set_link(form.addr, Some(link), Some(&display));
    }

    pub fn remove_link(&mut self, addr: CellAddr) {
        self.doc.set_link(addr, None, None);
    }

    pub fn follow_link(&mut self, addr: CellAddr) {
        let Some(link) = self.sheet().layout().links.get(&addr).cloned() else {
            return;
        };
        match link.kind() {
            LinkKind::Place => {
                let place = link.place().unwrap_or_default().to_owned();
                if !self.go_to(&place) {
                    self.dialog = Some(Dialog::Error {
                        title: text::app_name(),
                        message: text::bad_link_place().to_owned(),
                    });
                }
            }
            LinkKind::Web | LinkKind::Mail => open_external(&link.target),
            LinkKind::File => {
                self.dialog = Some(Dialog::FollowFile(link.target));
            }
        }
    }

    pub fn link_tip(&self, ctx: &Context, addr: CellAddr, cell: Rect) {
        let Some(link) = self.sheet().layout().links.get(&addr) else {
            return;
        };
        let shown = if link.tooltip.is_empty() {
            link.target.trim_start_matches('#').to_owned()
        } else {
            link.tooltip.clone()
        };
        Area::new(Id::unique("link-tip"))
            .order(Order::Tooltip)
            .interactable(false)
            .fixed_pos(cell.left_bottom() + vec2(theme::NOTE_GAP, theme::ISSUE_GAP))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(theme::ASSIST_HELP_WIDTH);
                    ui.label(RichText::new(shown).color(theme::chrome_text()));
                    ui.label(
                        RichText::new(text::follow_hint())
                            .font(theme::small_font())
                            .color(theme::chrome_text_muted()),
                    );
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(tab: LinkTab) -> LinkForm {
        LinkForm {
            addr: CellAddr::new(0, 0).unwrap(),
            tab,
            display: String::new(),
            address: "www.ornek.com".into(),
            sheet: 1,
            reference: "B4".into(),
            email: "ali@firma.com".into(),
            subject: "Aylık rapor".into(),
            tooltip: String::new(),
            existing: false,
        }
    }

    #[test]
    fn builds_link_targets() {
        let sheets = ["Özet".to_owned(), "Satış Verisi".to_owned()];
        assert_eq!(
            target(&form(LinkTab::Address), &sheets).unwrap(),
            "https://www.ornek.com"
        );
        assert_eq!(
            target(&form(LinkTab::Place), &sheets).unwrap(),
            "#'Satış Verisi'!B4"
        );
        let mail = target(&form(LinkTab::Mail), &sheets).unwrap();
        assert_eq!(mail, "mailto:ali@firma.com?subject=Ayl%C4%B1k%20rapor");
        assert_eq!(decode("Ayl%C4%B1k%20rapor"), "Aylık rapor");
    }
}
