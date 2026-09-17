use std::collections::BTreeMap;
use std::io;

use crate::contact::Contact;
use crate::fsutil::Dir;
use crate::paths::{uid_file_name, Layout, MAX_CONTACTS, MAX_VCARD_BYTES};
use crate::vcard::{now_rev, parse_card, serialize_card, split_cards, ParseError};

pub struct Store {
    pub layout: Layout,
    contacts: BTreeMap<String, Contact>,
}

impl Store {
    pub fn open() -> io::Result<Self> {
        let layout = Layout::open()?;
        let mut store = Self {
            layout,
            contacts: BTreeMap::new(),
        };
        store.reload()?;
        Ok(store)
    }

    pub fn reload(&mut self) -> io::Result<()> {
        self.contacts.clear();
        let names = self.layout.contacts.list_names()?;
        for name in names {
            if !name.ends_with(".vcf") {
                continue;
            }
            if self.contacts.len() >= MAX_CONTACTS {
                break;
            }
            match self.layout.contacts.read_capped(&name, MAX_VCARD_BYTES) {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    if let Ok(mut c) = parse_card(&text) {
                        if c.uid.is_empty() {
                            c.uid = name.trim_end_matches(".vcf").to_string();
                        }
                        self.contacts.insert(c.uid.clone(), c);
                    }
                }
                Err(_) => continue,
            }
        }
        Ok(())
    }

    pub fn all(&self) -> Vec<&Contact> {
        let mut v: Vec<&Contact> = self.contacts.values().collect();
        v.sort_by(|a, b| {
            a.display_name()
                .to_ascii_lowercase()
                .cmp(&b.display_name().to_ascii_lowercase())
        });
        v
    }

    pub fn search(&self, q: &str) -> Vec<&Contact> {
        self.all().into_iter().filter(|c| c.matches(q)).collect()
    }

    pub fn get(&self, uid: &str) -> Option<&Contact> {
        self.contacts.get(uid)
    }

    pub fn get_mut(&mut self, uid: &str) -> Option<&mut Contact> {
        self.contacts.get_mut(uid)
    }

    pub fn upsert(&mut self, mut c: Contact) -> io::Result<Contact> {
        c.normalize();
        c.rev = now_rev();
        if self.contacts.len() >= MAX_CONTACTS && !self.contacts.contains_key(&c.uid) {
            return Err(io::Error::other("too many contacts"));
        }
        self.write(&c)?;
        self.contacts.insert(c.uid.clone(), c.clone());
        Ok(c)
    }

    pub fn delete(&mut self, uid: &str) -> io::Result<bool> {
        let name = uid_file_name(uid)?;
        if self.contacts.remove(uid).is_none() {
            return Ok(false);
        }
        match self.layout.contacts.unlink(&name) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(true),
            Err(e) => Err(e),
        }
    }

    pub fn write(&self, c: &Contact) -> io::Result<()> {
        let name = uid_file_name(&c.uid)?;
        let text = serialize_card(c);
        self.layout.contacts.atomic_write(&name, text.as_bytes())
    }

    pub fn import_text(&mut self, text: &str) -> io::Result<ImportResult> {
        let mut result = ImportResult::default();
        for card in split_cards(text) {
            match parse_card(&card) {
                Ok(c) => {
                    let _ = self.upsert(c)?;
                    result.ok += 1;
                }
                Err(ParseError(_)) => result.skipped += 1,
            }
            if self.contacts.len() >= MAX_CONTACTS {
                break;
            }
        }
        Ok(result)
    }

    pub fn export_text(&self, uids: Option<&[String]>) -> String {
        let mut out = String::new();
        let list: Vec<&Contact> = match uids {
            Some(ids) if !ids.is_empty() => ids.iter().filter_map(|id| self.contacts.get(id)).collect(),
            _ => self.all(),
        };
        for c in list {
            out.push_str(&serialize_card(c));
        }
        out
    }

    pub fn count(&self) -> usize {
        self.contacts.len()
    }

    pub fn replace_from_remote(&mut self, c: Contact) -> io::Result<()> {
        let mut c = c;
        c.normalize();
        self.write(&c)?;
        self.contacts.insert(c.uid.clone(), c);
        Ok(())
    }

    pub fn contacts_dir(&self) -> &Dir {
        &self.layout.contacts
    }
}

#[derive(Default, serde::Serialize)]
pub struct ImportResult {
    pub ok: u32,
    pub skipped: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contact::Phone;

    #[test]
    fn crud_roundtrip_in_memory_shape() {
        let mut c = Contact {
            first: "Ada".into(),
            last: "Lovelace".into(),
            phones: vec![Phone {
                type_: "cell".into(),
                value: "+1".into(),
            }],
            ..Contact::default()
        };
        c.normalize();
        let text = serialize_card(&c);
        let back = parse_card(&text).unwrap();
        assert_eq!(back.first, "Ada");
        assert_eq!(back.primary_phone(), "+1");
    }
}
