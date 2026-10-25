use std::io::{Cursor, Read, Write};

use vellum_grid_core::protect::PasswordHash;
use vellum_grid_core::{Workbook, from_serial};
use zip::write::SimpleFileOptions;

use crate::Error;
use crate::ods::escape;

const THREADED_TYPE: &str =
    "http://schemas.microsoft.com/office/2017/10/relationships/threadedComment";
const PERSON_TYPE: &str = "http://schemas.microsoft.com/office/2017/10/relationships/person";
const THREADED_CONTENT: &str = "application/vnd.ms-excel.threadedcomments+xml";
const PERSON_CONTENT: &str = "application/vnd.ms-excel.person+xml";
const THREADED_NS: &str = "http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments";

pub fn guid(kind: u32, group: usize, index: usize) -> String {
    format!(
        "{{{kind:08X}-{:04X}-4000-8000-{index:012X}}}",
        group & 0xFFFF
    )
}

pub fn comment_id(sheet: usize, thread: usize, reply: usize) -> String {
    guid(0xC0FF_EE00, sheet, thread * 1000 + reply)
}

fn person_id(index: usize) -> String {
    guid(0xA0A0_0000, 0, index)
}

fn hash_attrs(hash: &PasswordHash, prefix: &str, legacy: &str) -> String {
    match hash {
        PasswordHash::Legacy(value) => format!(" {legacy}=\"{value:04X}\""),
        PasswordHash::Sha512 { hash, salt, spin } => format!(
            " {}=\"SHA-512\" {}=\"{hash}\" {}=\"{salt}\" {}=\"{spin}\"",
            key(prefix, "algorithmName"),
            key(prefix, "hashValue"),
            key(prefix, "saltValue"),
            key(prefix, "spinCount")
        ),
    }
}

fn key(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        let mut chars = name.chars();
        let first = chars
            .next()
            .map(|c| c.to_ascii_uppercase())
            .unwrap_or_default();
        format!("{prefix}{first}{}", chars.as_str())
    }
}

fn iso_time(serial: f64) -> String {
    match from_serial(serial) {
        Some(t) => format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.00",
            t.year, t.month, t.day, t.hour, t.minute, t.second
        ),
        None => "2000-01-01T00:00:00.00".to_owned(),
    }
}

fn authors(book: &Workbook) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for sheet in book.sheets() {
        for comment in sheet.layout().comments.values() {
            for reply in &comment.thread {
                if !out.contains(&reply.author) {
                    out.push(reply.author.clone());
                }
            }
        }
    }
    out
}

fn threaded_xml(book: &Workbook, sheet: usize, people: &[String]) -> String {
    let mut out = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><ThreadedComments xmlns="{THREADED_NS}" xmlns:x="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#
    );
    let comments = &book.sheets()[sheet].layout().comments;
    for (index, (addr, comment)) in comments.iter().enumerate() {
        let root = comment_id(sheet, index, 0);
        for (position, reply) in comment.thread.iter().enumerate() {
            let person = people
                .iter()
                .position(|name| *name == reply.author)
                .map(person_id)
                .unwrap_or_default();
            let id = comment_id(sheet, index, position);
            let parent = if position == 0 {
                String::new()
            } else {
                format!(" parentId=\"{root}\"")
            };
            let done = if position == 0 && comment.resolved {
                " done=\"1\""
            } else {
                ""
            };
            out.push_str(&format!(
                r#"<threadedComment ref="{addr}" dT="{}" personId="{person}" id="{id}"{parent}{done}><text>{}</text></threadedComment>"#,
                iso_time(reply.time),
                escape(&reply.text)
            ));
        }
    }
    out.push_str("</ThreadedComments>");
    out
}

fn persons_xml(people: &[String]) -> String {
    let mut out = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><personList xmlns="{THREADED_NS}" xmlns:x="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#
    );
    for (index, name) in people.iter().enumerate() {
        let name = escape(name);
        out.push_str(&format!(
            r#"<person displayName="{name}" id="{}" userId="{name}" providerId="None"/>"#,
            person_id(index)
        ));
    }
    out.push_str("</personList>");
    out
}

fn add_relation(xml: &str, id: &str, kind: &str, target: &str) -> String {
    let relation = format!(r#"<Relationship Id="{id}" Type="{kind}" Target="{target}"/>"#);
    xml.replacen(
        "</Relationships>",
        &format!("{relation}</Relationships>"),
        1,
    )
}

fn add_override(xml: &str, part: &str, content: &str) -> String {
    let item = format!(r#"<Override PartName="/{part}" ContentType="{content}"/>"#);
    xml.replacen("</Types>", &format!("{item}</Types>"), 1)
}

fn insert_attrs(xml: &str, tag: &str, attrs: &str) -> String {
    let open = format!("<{tag} ");
    match xml.find(&open) {
        Some(at) => {
            let mut out = xml.to_owned();
            out.insert_str(at + open.len(), &format!("{} ", attrs.trim()));
            out
        }
        None => xml.to_owned(),
    }
}

pub fn finish(bytes: Vec<u8>, book: &Workbook) -> Result<Vec<u8>, Error> {
    let people = authors(book);
    let threaded: Vec<usize> = (0..book.sheets().len())
        .filter(|index| !book.sheets()[*index].layout().comments.is_empty())
        .collect();
    let passwords = book.sheets().iter().any(|sheet| {
        sheet
            .layout()
            .protection
            .as_ref()
            .is_some_and(|p| p.password.is_some())
    });
    let drawn: Vec<usize> = (0..book.sheets().len())
        .filter(|index| {
            let layout = book.sheets()[*index].layout();
            !layout.pictures.is_empty() || !layout.shapes.is_empty()
        })
        .collect();
    if threaded.is_empty() && !passwords && book.protection.is_none() && drawn.is_empty() {
        return Ok(bytes);
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    let mut drawings = std::collections::HashMap::new();
    for sheet in drawn {
        let mut rels = String::new();
        let path = format!("xl/worksheets/_rels/sheet{}.xml.rels", sheet + 1);
        if let Ok(mut file) = archive.by_name(&path) {
            file.read_to_string(&mut rels)?;
        }
        if let Some(at) = rels.find("../drawings/") {
            let name: String = rels[at + 3..].chars().take_while(|c| *c != '"').collect();
            drawings.insert(format!("xl/{name}"), sheet);
        }
    }
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let path = file.name()?.into_owned();
        let mut content = Vec::new();
        file.read_to_end(&mut content)?;
        let text = || String::from_utf8_lossy(&content).into_owned();
        let changed: Option<String> = if let Some(sheet) = drawings.get(&path) {
            Some(crate::xlsx_drawing::rewrite(
                &text(),
                book.sheets()[*sheet].layout(),
            ))
        } else if path == "[Content_Types].xml" {
            let mut xml = text();
            for sheet in &threaded {
                xml = add_override(
                    &xml,
                    &format!("xl/threadedComments/threadedComment{}.xml", sheet + 1),
                    THREADED_CONTENT,
                );
            }
            if !threaded.is_empty() {
                xml = add_override(&xml, "xl/persons/person.xml", PERSON_CONTENT);
            }
            Some(xml)
        } else if path == "xl/_rels/workbook.xml.rels" && !threaded.is_empty() {
            Some(add_relation(
                &text(),
                "rIdPersons",
                PERSON_TYPE,
                "persons/person.xml",
            ))
        } else if path == "xl/workbook.xml" && book.protection.is_some() {
            let hash = book
                .protection
                .as_ref()
                .and_then(Option::as_ref)
                .map(|hash| hash_attrs(hash, "workbook", "workbookPassword"))
                .unwrap_or_default();
            let element = format!("<workbookProtection lockStructure=\"1\"{hash}/>");
            Some(text().replacen("<bookViews>", &format!("{element}<bookViews>"), 1))
        } else if let Some(number) = path
            .strip_prefix("xl/worksheets/_rels/sheet")
            .and_then(|rest| rest.strip_suffix(".xml.rels"))
            .and_then(|n| n.parse::<usize>().ok())
            .filter(|n| threaded.contains(&(n - 1)))
        {
            Some(add_relation(
                &text(),
                "rIdThreaded",
                THREADED_TYPE,
                &format!("../threadedComments/threadedComment{number}.xml"),
            ))
        } else if let Some(number) = path
            .strip_prefix("xl/worksheets/sheet")
            .and_then(|rest| rest.strip_suffix(".xml"))
            .and_then(|n| n.parse::<usize>().ok())
        {
            book.sheets()
                .get(number - 1)
                .and_then(|sheet| sheet.layout().protection.as_ref())
                .and_then(|protection| protection.password.as_ref())
                .map(|hash| {
                    insert_attrs(
                        &text(),
                        "sheetProtection",
                        &hash_attrs(hash, "", "password"),
                    )
                })
        } else {
            None
        };
        writer.start_file(path, options)?;
        match changed {
            Some(xml) => writer.write_all(xml.as_bytes())?,
            None => writer.write_all(&content)?,
        }
    }
    for sheet in &threaded {
        writer.start_file(
            format!("xl/threadedComments/threadedComment{}.xml", sheet + 1),
            options,
        )?;
        writer.write_all(threaded_xml(book, *sheet, &people).as_bytes())?;
    }
    if !threaded.is_empty() {
        writer.start_file("xl/persons/person.xml", options)?;
        writer.write_all(persons_xml(&people).as_bytes())?;
    }
    Ok(writer.finish()?.into_inner())
}
