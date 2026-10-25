use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read};

use quick_xml::events::BytesStart;
use vellum_grid_core::protect::{PasswordHash, Permissions, Protection};
use vellum_grid_core::review::{Comment, Link, Note, Reply};
use vellum_grid_core::{AutoFilter, Table, TableColumn, TotalFunction};
use vellum_grid_core::{CellAddr, Layout, date_serial, time_fraction};

use crate::xlsx_extras::{rels_path, resolve};
use crate::xlsx_format::{Tag, attr, name, walk};

type Archive<'a> = zip::ZipArchive<Cursor<&'a [u8]>>;

fn entry(archive: &mut Archive, path: &str) -> Option<String> {
    let mut text = String::new();
    archive.by_name(path).ok()?.read_to_string(&mut text).ok()?;
    Some(text)
}

pub(crate) struct Relation {
    pub id: String,
    pub kind: String,
    pub target: String,
}

pub(crate) fn relations(xml: &str) -> Vec<Relation> {
    let mut out = Vec::new();
    walk(xml, |tag| {
        if let Tag::Open(e) | Tag::Empty(e) = tag
            && name(&e) == "Relationship"
            && let (Some(id), Some(target)) = (attr(&e, "Id"), attr(&e, "Target"))
        {
            out.push(Relation {
                id,
                kind: attr(&e, "Type").unwrap_or_default(),
                target,
            });
        }
    });
    out
}

fn book_hash(e: &BytesStart) -> Option<PasswordHash> {
    if let Some(hash) = attr(e, "workbookHashValue") {
        return Some(PasswordHash::Sha512 {
            hash,
            salt: attr(e, "workbookSaltValue").unwrap_or_default(),
            spin: attr(e, "workbookSpinCount")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        });
    }
    attr(e, "workbookPassword")
        .and_then(|hex| u16::from_str_radix(&hex, 16).ok())
        .map(PasswordHash::Legacy)
}

fn protection(e: &BytesStart) -> Protection {
    let allowed = |key: &str| attr(e, key).as_deref() == Some("0");
    let open = |key: &str| attr(e, key).as_deref() != Some("1");
    Protection {
        password: sheet_hash(e),
        allow: Permissions {
            select_locked: open("selectLockedCells"),
            select_unlocked: open("selectUnlockedCells"),
            format_cells: allowed("formatCells"),
            format_columns: allowed("formatColumns"),
            format_rows: allowed("formatRows"),
            insert_columns: allowed("insertColumns"),
            insert_rows: allowed("insertRows"),
            insert_links: allowed("insertHyperlinks"),
            delete_columns: allowed("deleteColumns"),
            delete_rows: allowed("deleteRows"),
            sort: allowed("sort"),
            autofilter: allowed("autoFilter"),
            edit_objects: open("objects"),
        },
    }
}

fn sheet_hash(e: &BytesStart) -> Option<PasswordHash> {
    if let Some(hash) = attr(e, "hashValue") {
        return Some(PasswordHash::Sha512 {
            hash,
            salt: attr(e, "saltValue").unwrap_or_default(),
            spin: attr(e, "spinCount")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        });
    }
    attr(e, "password")
        .and_then(|hex| u16::from_str_radix(&hex, 16).ok())
        .map(PasswordHash::Legacy)
}

fn notes(xml: &str) -> BTreeMap<CellAddr, (String, String)> {
    let mut authors = Vec::new();
    let mut out = BTreeMap::new();
    let mut in_author = false;
    let mut current: Option<(CellAddr, usize, String)> = None;
    walk(xml, |tag| match tag {
        Tag::Open(e) if name(&e) == "author" => in_author = true,
        Tag::Open(e) if name(&e) == "comment" => {
            current = attr(&e, "ref").and_then(|r| r.parse().ok()).map(|addr| {
                let author = attr(&e, "authorId")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                (addr, author, String::new())
            });
        }
        Tag::Text(text) => {
            if in_author {
                authors.push(text);
            } else if let Some((_, _, body)) = current.as_mut() {
                body.push_str(&text);
            }
        }
        Tag::Close(tag) if tag == "author" => in_author = false,
        Tag::Close(tag) if tag == "comment" => {
            if let Some((addr, author, body)) = current.take() {
                let author = authors.get(author).cloned().unwrap_or_default();
                out.insert(addr, (author, body));
            }
        }
        _ => {}
    });
    out
}

fn persons(xml: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    walk(xml, |tag| {
        if let Tag::Open(e) | Tag::Empty(e) = tag
            && name(&e) == "person"
            && let (Some(id), Some(display)) = (attr(&e, "id"), attr(&e, "displayName"))
        {
            out.insert(id, display);
        }
    });
    out
}

fn parse_time(text: &str) -> f64 {
    let digits: Vec<u32> = text
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .take(6)
        .filter_map(|part| part.parse().ok())
        .collect();
    let [year, month, day, hour, minute, second] = match digits.as_slice() {
        [y, mo, d, h, mi, s, ..] => [*y, *mo, *d, *h, *mi, *s],
        [y, mo, d] => [*y, *mo, *d, 0, 0, 0],
        _ => return 0.0,
    };
    date_serial(i64::from(year), month, day).unwrap_or(0.0)
        + time_fraction(hour, minute, second).unwrap_or(0.0)
}

fn threads(xml: &str, people: &HashMap<String, String>) -> BTreeMap<CellAddr, Comment> {
    struct Entry {
        addr: CellAddr,
        id: String,
        parent: Option<String>,
        reply: Reply,
        done: bool,
    }
    let mut entries: Vec<Entry> = Vec::new();
    let mut current: Option<Entry> = None;
    walk(xml, |tag| match tag {
        Tag::Open(e) if name(&e) == "threadedComment" => {
            current = attr(&e, "ref")
                .and_then(|r| r.parse().ok())
                .map(|addr| Entry {
                    addr,
                    id: attr(&e, "id").unwrap_or_default(),
                    parent: attr(&e, "parentId"),
                    reply: Reply {
                        author: attr(&e, "personId")
                            .and_then(|id| people.get(&id).cloned())
                            .unwrap_or_default(),
                        text: String::new(),
                        time: attr(&e, "dT").map_or(0.0, |t| parse_time(&t)),
                    },
                    done: attr(&e, "done").as_deref() == Some("1"),
                });
        }
        Tag::Text(text) => {
            if let Some(entry) = current.as_mut() {
                entry.reply.text.push_str(&text);
            }
        }
        Tag::Close(tag) if tag == "threadedComment" => {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
        }
        _ => {}
    });
    let mut out: BTreeMap<CellAddr, Comment> = BTreeMap::new();
    for entry in entries.iter().filter(|e| e.parent.is_none()) {
        let mut thread = vec![entry.reply.clone()];
        thread.extend(
            entries
                .iter()
                .filter(|reply| reply.parent.as_deref() == Some(entry.id.as_str()))
                .map(|reply| reply.reply.clone()),
        );
        out.insert(
            entry.addr,
            Comment {
                thread,
                resolved: entry.done,
            },
        );
    }
    out
}

pub fn read_sheet(archive: &mut Archive, path: &str, xml: &str, layout: &mut Layout) {
    let rels = entry(archive, &rels_path(path))
        .map(|text| relations(&text))
        .unwrap_or_default();
    let mut links = Vec::new();
    walk(xml, |tag| {
        if let Tag::Open(e) | Tag::Empty(e) = tag {
            match name(&e).as_str() {
                "sheetProtection" if attr(&e, "sheet").as_deref() == Some("1") => {
                    layout.protection = Some(protection(&e));
                }
                "hyperlink" => {
                    let target = match (attr(&e, "id"), attr(&e, "location")) {
                        (Some(id), location) => {
                            rels.iter()
                                .find(|rel| rel.id == id)
                                .map(|rel| match location {
                                    Some(location) => format!("{}#{location}", rel.target),
                                    None => rel.target.clone(),
                                })
                        }
                        (None, Some(location)) => Some(format!("#{location}")),
                        _ => None,
                    };
                    if let (Some(range), Some(target)) = (attr(&e, "ref"), target) {
                        let start = range.split(':').next().unwrap_or_default().to_owned();
                        links.push((start, target, attr(&e, "tooltip").unwrap_or_default()));
                    }
                }
                _ => {}
            }
        }
    });
    for (cell, target, tooltip) in links {
        if let Ok(addr) = cell.parse::<CellAddr>() {
            layout.links.insert(addr, Link { target, tooltip });
        }
    }
    let part = |kind: &str| {
        rels.iter()
            .find(|rel| rel.kind.ends_with(kind))
            .map(|rel| resolve(path, &rel.target))
    };
    if let Some(comments) = part("/comments").and_then(|p| entry(archive, &p)) {
        for (addr, (author, text)) in notes(&comments) {
            if author.starts_with("tc=") {
                continue;
            }
            let prefix = format!("{author}:");
            let text = text
                .strip_prefix(&prefix)
                .map_or(text.as_str(), |rest| rest.trim_start_matches(['\n', '\r']))
                .to_owned();
            layout.notes.insert(
                addr,
                Note {
                    author,
                    text,
                    visible: false,
                },
            );
        }
    }
    let tables: Vec<String> = rels
        .iter()
        .filter(|rel| rel.kind.ends_with("/table"))
        .map(|rel| resolve(path, &rel.target))
        .collect();
    for table in tables
        .iter()
        .filter_map(|p| entry(archive, p))
        .filter_map(|x| table(&x))
    {
        if table.filter && layout.filter.is_none() {
            layout.filter = Some(AutoFilter {
                range: table.filter_range(),
                criteria: Default::default(),
            });
        }
        layout.tables.push(table);
    }
    if let Some(threaded) = part("/threadedComment").and_then(|p| entry(archive, &p)) {
        let people = person_list(archive);
        layout.comments = threads(&threaded, &people);
    }
}

fn table(xml: &str) -> Option<Table> {
    let mut found: Option<Table> = None;
    walk(xml, |tag| {
        let (Tag::Open(e) | Tag::Empty(e)) = tag else {
            return;
        };
        let on = |key: &str| attr(&e, key).as_deref() == Some("1");
        match (name(&e).as_str(), found.as_mut()) {
            ("table", None) => {
                let range = attr(&e, "ref").and_then(|r| r.parse().ok());
                let label = attr(&e, "displayName").or_else(|| attr(&e, "name"));
                if let (Some(range), Some(label)) = (range, label) {
                    let mut table = Table::new(label, range, Vec::new());
                    table.header = attr(&e, "headerRowCount").as_deref() != Some("0");
                    table.totals = attr(&e, "totalsRowCount")
                        .and_then(|v| v.parse::<u32>().ok())
                        .is_some_and(|n| n > 0);
                    table.filter = false;
                    table.style = String::new();
                    table.banded_rows = false;
                    found = Some(table);
                }
            }
            ("autoFilter", Some(table)) => table.filter = true,
            ("tableColumn", Some(table)) => {
                let mut column = TableColumn::new(attr(&e, "name").unwrap_or_default());
                column.total = attr(&e, "totalsRowFunction")
                    .map_or(TotalFunction::None, |key| TotalFunction::from_key(&key));
                column.label = attr(&e, "totalsRowLabel");
                table.columns.push(column);
            }
            ("tableStyleInfo", Some(table)) => {
                table.style = attr(&e, "name").unwrap_or_default();
                table.first_col = on("showFirstColumn");
                table.last_col = on("showLastColumn");
                table.banded_rows = on("showRowStripes");
                table.banded_cols = on("showColumnStripes");
            }
            _ => {}
        }
    });
    found.filter(|t| t.columns.len() == t.range.cols() as usize)
}

fn person_list(archive: &mut Archive) -> HashMap<String, String> {
    let rels = entry(archive, "xl/_rels/workbook.xml.rels")
        .map(|text| relations(&text))
        .unwrap_or_default();
    rels.iter()
        .find(|rel| rel.kind.ends_with("/person"))
        .and_then(|rel| entry(archive, &resolve("xl/workbook.xml", &rel.target)))
        .map(|xml| persons(&xml))
        .unwrap_or_default()
}

pub fn book_protection(bytes: &[u8]) -> Option<Option<PasswordHash>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let xml = entry(&mut archive, "xl/workbook.xml")?;
    let mut found = None;
    walk(&xml, |tag| {
        if let Tag::Open(e) | Tag::Empty(e) = tag
            && name(&e) == "workbookProtection"
            && attr(&e, "lockStructure").as_deref() == Some("1")
        {
            found = Some(book_hash(&e));
        }
    });
    found
}
