use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Phone {
    #[serde(rename = "type")]
    pub type_: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Email {
    #[serde(rename = "type")]
    pub type_: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Address {
    #[serde(rename = "type")]
    pub type_: String,
    pub street: String,
    pub city: String,
    pub region: String,
    pub postal: String,
    pub country: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Link {
    #[serde(rename = "type")]
    pub type_: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Contact {
    pub uid: String,
    pub first: String,
    pub last: String,
    #[serde(rename = "fn")]
    pub fn_: String,
    pub nickname: String,
    pub org: String,
    pub title: String,
    pub note: String,
    #[serde(default)]
    pub bday: String,
    pub phones: Vec<Phone>,
    pub emails: Vec<Email>,
    #[serde(default)]
    pub addresses: Vec<Address>,
    #[serde(default)]
    pub urls: Vec<Link>,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub is_group: bool,
    pub rev: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo_jpeg: Option<Vec<u8>>,
    #[serde(default)]
    pub has_photo: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo_b64: Option<String>,
    #[serde(skip)]
    pub etag: Option<String>,
    #[serde(skip)]
    pub href: Option<String>,
    #[serde(skip)]
    pub photo_path: Option<String>,
}

impl Contact {
    pub fn normalize(&mut self) {
        if self.uid.is_empty() {
            self.uid = uuid::Uuid::new_v4().to_string();
        }
        self.first = self.first.trim().to_string();
        self.last = self.last.trim().to_string();
        self.nickname = self.nickname.trim().to_string();
        self.org = self.org.trim().to_string();
        self.title = self.title.trim().to_string();
        self.note = self.note.trim().to_string();
        self.bday = self.bday.trim().to_string();
        if self.fn_.trim().is_empty() {
            self.fn_ = self.display_name();
        } else {
            self.fn_ = self.fn_.trim().to_string();
        }
        self.phones.retain(|p| !p.value.trim().is_empty());
        self.emails.retain(|e| !e.value.trim().is_empty());
        self.urls.retain(|u| !u.value.trim().is_empty());
        self.addresses.retain(|a| {
            !a.street.trim().is_empty()
                || !a.city.trim().is_empty()
                || !a.region.trim().is_empty()
                || !a.postal.trim().is_empty()
                || !a.country.trim().is_empty()
        });
        for p in &mut self.phones {
            p.value = p.value.trim().to_string();
            if p.type_.is_empty() {
                p.type_ = "other".into();
            }
        }
        for e in &mut self.emails {
            e.value = e.value.trim().to_string();
            if e.type_.is_empty() {
                e.type_ = "other".into();
            }
        }
        self.has_photo = self.photo_jpeg.as_ref().is_some_and(|p| !p.is_empty());
    }

    pub fn display_name(&self) -> String {
        let composed = format!("{} {}", self.first, self.last)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !composed.is_empty() {
            composed
        } else if !self.fn_.trim().is_empty() {
            self.fn_.trim().to_string()
        } else if !self.org.is_empty() {
            self.org.clone()
        } else if !self.nickname.is_empty() {
            self.nickname.clone()
        } else {
            "Unnamed".into()
        }
    }

    pub fn primary_phone(&self) -> String {
        self.phones
            .first()
            .map(|p| p.value.clone())
            .unwrap_or_default()
    }

    pub fn for_list(&self) -> serde_json::Value {
        serde_json::json!({
            "uid": self.uid,
            "fn": self.display_name(),
            "first": self.first,
            "last": self.last,
            "nickname": self.nickname,
            "org": self.org,
            "title": self.title,
            "phone": self.primary_phone(),
            "email": self.emails.first().map(|e| e.value.clone()).unwrap_or_default(),
            "note": self.note,
            "bday": self.bday,
            "phones": self.phones,
            "emails": self.emails,
            "addresses": self.addresses,
            "urls": self.urls,
            "groups": self.groups,
            "members": self.members,
            "is_group": self.is_group,
            "has_photo": self.has_photo,
            "photo_file": self.photo_path.clone().unwrap_or_default(),
        })
    }

    pub fn for_detail(&self) -> serde_json::Value {
        let photo_b64 = self.photo_jpeg.as_ref().map(|p| {
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, p)
        });
        serde_json::json!({
            "uid": self.uid,
            "fn": self.display_name(),
            "first": self.first,
            "last": self.last,
            "nickname": self.nickname,
            "org": self.org,
            "title": self.title,
            "note": self.note,
            "bday": self.bday,
            "phones": self.phones,
            "emails": self.emails,
            "addresses": self.addresses,
            "urls": self.urls,
            "groups": self.groups,
            "members": self.members,
            "is_group": self.is_group,
            "rev": self.rev,
            "has_photo": self.has_photo,
            "photo_file": self.photo_path.clone().unwrap_or_default(),
            "photo_b64": photo_b64,
        })
    }

    pub fn matches(&self, q: &str) -> bool {
        if q.is_empty() {
            return true;
        }
        let q = q.to_ascii_lowercase();
        let hay = [
            self.display_name(),
            self.first.clone(),
            self.last.clone(),
            self.nickname.clone(),
            self.org.clone(),
            self.title.clone(),
            self.note.clone(),
            self.bday.clone(),
        ];
        if hay.iter().any(|h| h.to_ascii_lowercase().contains(&q)) {
            return true;
        }
        self.phones
            .iter()
            .any(|p| p.value.to_ascii_lowercase().contains(&q))
            || self
                .emails
                .iter()
                .any(|e| e.value.to_ascii_lowercase().contains(&q))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_fields() {
        let c = Contact {
            first: "Marie".into(),
            last: "Curie".into(),
            org: "Radium".into(),
            emails: vec![Email {
                type_: "work".into(),
                value: "marie@lab.fr".into(),
            }],
            phones: vec![Phone {
                type_: "cell".into(),
                value: "+331".into(),
            }],
            ..Contact::default()
        };
        assert!(c.matches("ma"));
        assert!(c.matches("lab.fr"));
        assert!(c.matches("+331"));
        assert!(c.matches("rad"));
        assert!(!c.matches("zzz"));
    }
}
