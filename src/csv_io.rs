use std::io;

use crate::contact::{Contact, Email, Phone};
use crate::store::Store;

pub fn import_csv(store: &mut Store, text: &str) -> io::Result<(u32, u32)> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers = rdr
        .headers()
        .map_err(io::Error::other)?
        .iter()
        .map(|h| normalize_header(h))
        .collect::<Vec<_>>();
    let mut ok = 0u32;
    let mut skipped = 0u32;
    for rec in rdr.records() {
        let rec = match rec {
            Ok(r) => r,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        let mut c = Contact::default();
        for (i, field) in rec.iter().enumerate() {
            let Some(h) = headers.get(i) else { continue };
            apply_field(&mut c, h, field);
        }
        c.normalize();
        if c.first.is_empty() && c.last.is_empty() && c.fn_.is_empty() {
            skipped += 1;
            continue;
        }
        store.upsert(c)?;
        ok += 1;
    }
    Ok((ok, skipped))
}

pub fn export_csv(store: &Store, uids: Option<&[String]>) -> io::Result<String> {
    let mut w = csv::Writer::from_writer(Vec::new());
    w.write_record([
        "First Name",
        "Last Name",
        "Nickname",
        "Organization",
        "Title",
        "Notes",
        "E-mail Address",
        "E-mail 2 - Value",
        "Phone 1 - Value",
        "Phone 2 - Value",
        "Phone 1 - Type",
        "Phone 2 - Type",
    ])
    .map_err(io::Error::other)?;
    let list = match uids {
        Some(ids) if !ids.is_empty() => ids.iter().filter_map(|id| store.get(id)).collect::<Vec<_>>(),
        _ => store.all(),
    };
    for c in list {
        w.write_record([
            c.first.as_str(),
            c.last.as_str(),
            c.nickname.as_str(),
            c.org.as_str(),
            c.title.as_str(),
            c.note.as_str(),
            c.emails.first().map(|e| e.value.as_str()).unwrap_or(""),
            c.emails.get(1).map(|e| e.value.as_str()).unwrap_or(""),
            c.phones.first().map(|p| p.value.as_str()).unwrap_or(""),
            c.phones.get(1).map(|p| p.value.as_str()).unwrap_or(""),
            c.phones.first().map(|p| p.type_.as_str()).unwrap_or(""),
            c.phones.get(1).map(|p| p.type_.as_str()).unwrap_or(""),
        ])
        .map_err(io::Error::other)?;
    }
    w.flush().map_err(io::Error::other)?;
    let bytes = w.into_inner().map_err(io::Error::other)?;
    String::from_utf8(bytes).map_err(|_| io::Error::other("csv not utf8"))
}

fn normalize_header(h: &str) -> String {
    h.trim().to_ascii_lowercase()
}

fn apply_field(c: &mut Contact, header: &str, value: &str) {
    let v = value.trim();
    if v.is_empty() {
        return;
    }
    match header {
        "first name" | "firstname" | "given name" | "first" => c.first = v.into(),
        "last name" | "lastname" | "family name" | "last" => c.last = v.into(),
        "name" | "full name" | "fn" => c.fn_ = v.into(),
        "nickname" | "nick" => c.nickname = v.into(),
        "organization" | "org" | "company" => c.org = v.into(),
        "title" | "job title" | "jobtitle" => c.title = v.into(),
        "notes" | "note" => c.note = v.into(),
        h if h.contains("e-mail") || h.contains("email") => {
            c.emails.push(Email {
                type_: if h.contains("work") {
                    "work".into()
                } else if h.contains("home") {
                    "home".into()
                } else {
                    "other".into()
                },
                value: v.into(),
            });
        }
        h if h.contains("phone") || h.contains("mobile") || h.contains("tel") => {
            if h.contains("type") {
                if let Some(last) = c.phones.last_mut() {
                    last.type_ = v.to_ascii_lowercase();
                }
            } else {
                c.phones.push(Phone {
                    type_: if h.contains("mobile") || h.contains("cell") {
                        "cell".into()
                    } else if h.contains("work") {
                        "work".into()
                    } else if h.contains("home") {
                        "home".into()
                    } else {
                        "other".into()
                    },
                    value: v.into(),
                });
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_headers_map() {
        let mut c = Contact::default();
        apply_field(&mut c, "first name", "Ada");
        apply_field(&mut c, "e-mail address", "ada@lab");
        apply_field(&mut c, "phone 1 - value", "+1");
        c.normalize();
        assert_eq!(c.first, "Ada");
        assert_eq!(c.emails[0].value, "ada@lab");
        assert_eq!(c.phones[0].value, "+1");
    }
}
