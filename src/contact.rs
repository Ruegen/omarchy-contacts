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
    #[serde(default)]
    pub middle: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub suffix: String,
    #[serde(rename = "fn")]
    pub fn_: String,
    pub nickname: String,
    pub org: String,
    #[serde(default)]
    pub department: String,
    pub title: String,
    #[serde(default)]
    pub role: String,
    pub note: String,
    #[serde(default)]
    pub bday: String,
    #[serde(default)]
    pub anniversary: String,
    pub phones: Vec<Phone>,
    pub emails: Vec<Email>,
    #[serde(default)]
    pub addresses: Vec<Address>,
    #[serde(default)]
    pub urls: Vec<Link>,
    #[serde(default)]
    pub ims: Vec<Link>,
    #[serde(default)]
    pub socials: Vec<Link>,
    #[serde(default)]
    pub related: Vec<Link>,
    #[serde(default)]
    pub dates: Vec<Link>,
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
        self.middle = self.middle.trim().to_string();
        self.prefix = self.prefix.trim().to_string();
        self.suffix = self.suffix.trim().to_string();
        self.nickname = self.nickname.trim().to_string();
        self.org = self.org.trim().to_string();
        self.department = self.department.trim().to_string();
        self.title = self.title.trim().to_string();
        self.role = self.role.trim().to_string();
        self.note = self.note.trim().to_string();
        self.bday = self.bday.trim().to_string();
        self.anniversary = self.anniversary.trim().to_string();
        if self.fn_.trim().is_empty() {
            self.fn_ = self.display_name();
        } else {
            self.fn_ = self.fn_.trim().to_string();
        }
        self.phones.retain(|p| !p.value.trim().is_empty());
        self.emails.retain(|e| !e.value.trim().is_empty());
        self.urls.retain(|u| !u.value.trim().is_empty());
        self.ims.retain(|u| !u.value.trim().is_empty());
        self.socials.retain(|u| !u.value.trim().is_empty());
        self.related.retain(|u| !u.value.trim().is_empty());
        self.dates.retain(|u| !u.value.trim().is_empty());
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
        for list in [
            &mut self.urls,
            &mut self.ims,
            &mut self.socials,
            &mut self.related,
            &mut self.dates,
        ] {
            for item in list {
                item.value = item.value.trim().to_string();
                if item.type_.is_empty() {
                    item.type_ = "other".into();
                }
            }
        }
        self.has_photo = self.photo_jpeg.as_ref().is_some_and(|p| !p.is_empty());
    }

    pub fn display_name(&self) -> String {
        let composed = [
            self.prefix.as_str(),
            self.first.as_str(),
            self.middle.as_str(),
            self.last.as_str(),
            self.suffix.as_str(),
        ]
        .into_iter()
        .filter(|s| !s.trim().is_empty())
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

    fn payload(&self, photo_b64: Option<String>) -> serde_json::Value {
        serde_json::json!({
            "uid": self.uid,
            "fn": self.display_name(),
            "first": self.first,
            "last": self.last,
            "middle": self.middle,
            "prefix": self.prefix,
            "suffix": self.suffix,
            "nickname": self.nickname,
            "org": self.org,
            "department": self.department,
            "title": self.title,
            "role": self.role,
            "note": self.note,
            "bday": self.bday,
            "anniversary": self.anniversary,
            "phone": self.primary_phone(),
            "email": self.emails.first().map(|e| e.value.clone()).unwrap_or_default(),
            "phones": self.phones,
            "emails": self.emails,
            "addresses": self.addresses,
            "urls": self.urls,
            "ims": self.ims,
            "socials": self.socials,
            "related": self.related,
            "dates": self.dates,
            "groups": self.groups,
            "members": self.members,
            "is_group": self.is_group,
            "rev": self.rev,
            "has_photo": self.has_photo,
            "photo_file": self.photo_path.clone().unwrap_or_default(),
            "photo_b64": photo_b64,
        })
    }

    pub fn for_list(&self) -> serde_json::Value {
        self.payload(None)
    }

    pub fn for_detail(&self) -> serde_json::Value {
        let photo_b64 = self.photo_jpeg.as_ref().map(|p| {
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, p)
        });
        self.payload(photo_b64)
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
            self.middle.clone(),
            self.prefix.clone(),
            self.suffix.clone(),
            self.nickname.clone(),
            self.org.clone(),
            self.department.clone(),
            self.title.clone(),
            self.role.clone(),
            self.note.clone(),
            self.bday.clone(),
            self.anniversary.clone(),
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
            || self
                .urls
                .iter()
                .chain(self.ims.iter())
                .chain(self.socials.iter())
                .chain(self.related.iter())
                .any(|u| u.value.to_ascii_lowercase().contains(&q))
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

    #[test]
    fn display_includes_name_parts() {
        let c = Contact {
            prefix: "Dr.".into(),
            first: "Marie".into(),
            middle: "Skłodowska".into(),
            last: "Curie".into(),
            suffix: "PhD".into(),
            ..Contact::default()
        };
        assert_eq!(c.display_name(), "Dr. Marie Skłodowska Curie PhD");
    }
}
