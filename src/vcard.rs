//! vCard 3.0 / 4.0 subset used by the address book.

use std::collections::HashMap;

use crate::contact::{Address, Contact, Email, Link, Phone};
use crate::paths::MAX_PHOTO_BYTES;

#[derive(Clone, Copy)]
enum GroupField {
    Phone(usize),
    Email(usize),
    Url(usize),
    Address(usize),
    Im(usize),
    Social(usize),
    Related(usize),
}

#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn split_cards(input: &str) -> Vec<String> {
    let mut cards = Vec::new();
    let mut buf = String::new();
    let mut inside = false;
    for raw in input.lines() {
        let line = raw.trim_end_matches('\r');
        let upper = line.to_ascii_uppercase();
        if upper == "BEGIN:VCARD" {
            inside = true;
            buf.clear();
            buf.push_str(line);
            buf.push('\n');
            continue;
        }
        if !inside {
            continue;
        }
        buf.push_str(line);
        buf.push('\n');
        if upper == "END:VCARD" {
            cards.push(std::mem::take(&mut buf));
            inside = false;
        }
    }
    cards
}

pub fn parse_card(text: &str) -> Result<Contact, ParseError> {
    let unfolded = unfold(&decode_markup(text));
    let mut c = Contact::default();
    let mut field_by_group: HashMap<String, GroupField> = HashMap::new();
    let mut label_by_group: HashMap<String, String> = HashMap::new();
    let mut date_by_group: HashMap<String, String> = HashMap::new();
    for line in unfolded.lines() {
        if line.is_empty() {
            continue;
        }
        let upper = line.to_ascii_uppercase();
        if upper == "BEGIN:VCARD" || upper == "END:VCARD" {
            continue;
        }
        let Some((raw_name, params, value)) = split_prop(line) else {
            continue;
        };
        let (group, name) = split_grouped(&raw_name);
        let name = name.to_ascii_uppercase();
        match name.as_str() {
            "VERSION" => {}
            "UID" => c.uid = unescape(&value),
            "FN" => c.fn_ = unescape(&value),
            "N" => apply_n(&mut c, &value),
            "NICKNAME" => c.nickname = unescape(&value),
            "ORG" => apply_org(&mut c, &value),
            "TITLE" => c.title = unescape(&value),
            "ROLE" => c.role = unescape(&value),
            "NOTE" => c.note = decode_note(&params, &value),
            "BDAY" => c.bday = unescape(&value),
            "ANNIVERSARY" => c.anniversary = unescape(&value),
            "REV" => c.rev = unescape(&value),
            "TEL" => {
                let v = normalize_tel(&unescape(&value));
                if !v.is_empty() {
                    c.phones.push(Phone {
                        type_: tel_type(&params),
                        value: v,
                    });
                    remember_group(
                        &group,
                        GroupField::Phone(c.phones.len() - 1),
                        &mut field_by_group,
                        &label_by_group,
                        &mut c,
                    );
                }
            }
            "EMAIL" => {
                let v = unescape(&value);
                if !v.is_empty() {
                    c.emails.push(Email {
                        type_: email_type(&params),
                        value: v,
                    });
                    remember_group(
                        &group,
                        GroupField::Email(c.emails.len() - 1),
                        &mut field_by_group,
                        &label_by_group,
                        &mut c,
                    );
                }
            }
            "ADR" => {
                if let Some(addr) = parse_adr(&params, &value) {
                    c.addresses.push(addr);
                    remember_group(
                        &group,
                        GroupField::Address(c.addresses.len() - 1),
                        &mut field_by_group,
                        &label_by_group,
                        &mut c,
                    );
                }
            }
            "URL" => {
                let v = unescape(&value);
                if !v.is_empty() {
                    c.urls.push(Link {
                        type_: url_type(&params),
                        value: v,
                    });
                    remember_group(
                        &group,
                        GroupField::Url(c.urls.len() - 1),
                        &mut field_by_group,
                        &label_by_group,
                        &mut c,
                    );
                }
            }
            "IMPP" => {
                let v = unescape(&value);
                if !v.is_empty() {
                    c.ims.push(Link {
                        type_: im_type(&params, &v),
                        value: strip_im_scheme(&v),
                    });
                    remember_group(
                        &group,
                        GroupField::Im(c.ims.len() - 1),
                        &mut field_by_group,
                        &label_by_group,
                        &mut c,
                    );
                }
            }
            "X-SOCIALPROFILE" => {
                let v = unescape(&value);
                if !v.is_empty() {
                    c.socials.push(Link {
                        type_: social_type(&params),
                        value: v,
                    });
                    remember_group(
                        &group,
                        GroupField::Social(c.socials.len() - 1),
                        &mut field_by_group,
                        &label_by_group,
                        &mut c,
                    );
                }
            }
            "RELATED" | "X-ABRELATEDNAMES" => {
                let v = unescape(&value);
                if !v.is_empty() {
                    c.related.push(Link {
                        type_: related_type(&params),
                        value: v,
                    });
                    remember_group(
                        &group,
                        GroupField::Related(c.related.len() - 1),
                        &mut field_by_group,
                        &label_by_group,
                        &mut c,
                    );
                }
            }
            "X-ABDATE" => {
                let v = unescape(&value);
                if !v.is_empty() && !group.is_empty() {
                    date_by_group.insert(group.clone(), v);
                } else if !v.is_empty() && c.anniversary.is_empty() {
                    c.anniversary = v;
                }
            }
            "X-ABLABEL" => {
                let label = decode_ab_label(&unescape(&value));
                if !group.is_empty() && !label.is_empty() {
                    label_by_group.insert(group.clone(), label.clone());
                    apply_group_label(&mut c, &field_by_group, &group, &label);
                }
            }
            "X-AIM" | "X-ICQ" | "X-JABBER" | "X-MSN" | "X-YAHOO" | "X-SKYPE"
            | "X-GOOGLE-TALK" | "X-GTALK" => {
                let v = unescape(&value);
                if !v.is_empty() {
                    c.ims.push(Link {
                        type_: name
                            .trim_start_matches("X-")
                            .replace("GOOGLE-TALK", "gtalk")
                            .to_ascii_lowercase(),
                        value: v,
                    });
                }
            }
            "CATEGORIES" => {
                for part in unescape(&value).split(',') {
                    let g = part.trim();
                    if !g.is_empty() && !c.groups.iter().any(|x| x.eq_ignore_ascii_case(g)) {
                        c.groups.push(g.to_string());
                    }
                }
            }
            "KIND" | "X-ADDRESSBOOKSERVER-KIND" => {
                if unescape(&value).eq_ignore_ascii_case("group") {
                    c.is_group = true;
                }
            }
            "MEMBER" | "X-ADDRESSBOOKSERVER-MEMBER" => {
                let m = member_uid(&unescape(&value));
                if !m.is_empty() {
                    c.members.push(m);
                    c.is_group = true;
                }
            }
            "PHOTO" => c.photo_jpeg = parse_photo(&params, &value),
            _ => {}
        }
    }
    for (group, value) in date_by_group {
        let label = label_by_group
            .get(&group)
            .map(|s| s.as_str())
            .unwrap_or("date");
        if label.eq_ignore_ascii_case("anniversary") && c.anniversary.is_empty() {
            c.anniversary = value;
        } else if !label.eq_ignore_ascii_case("anniversary") {
            c.dates.push(Link {
                type_: label.to_string(),
                value,
            });
        }
    }
    c.normalize();
    if c.fn_.is_empty() && c.first.is_empty() && c.last.is_empty() {
        return Err(ParseError("empty contact".into()));
    }
    Ok(c)
}

pub fn serialize_card(c: &Contact) -> String {
    let mut out = String::from("BEGIN:VCARD\r\nVERSION:3.0\r\n");
    push_prop(&mut out, "UID", &c.uid);
    push_prop(&mut out, "FN", &c.display_name());
    out.push_str("N:");
    out.push_str(&escape(&c.last));
    out.push(';');
    out.push_str(&escape(&c.first));
    out.push(';');
    out.push_str(&escape(&c.middle));
    out.push(';');
    out.push_str(&escape(&c.prefix));
    out.push(';');
    out.push_str(&escape(&c.suffix));
    out.push_str("\r\n");
    if !c.nickname.is_empty() {
        push_prop(&mut out, "NICKNAME", &c.nickname);
    }
    if !c.org.is_empty() || !c.department.is_empty() {
        if c.department.is_empty() {
            push_prop(&mut out, "ORG", &c.org);
        } else {
            out.push_str("ORG:");
            out.push_str(&escape(&c.org));
            out.push(';');
            out.push_str(&escape(&c.department));
            out.push_str("\r\n");
        }
    }
    if !c.title.is_empty() {
        push_prop(&mut out, "TITLE", &c.title);
    }
    if !c.role.is_empty() {
        push_prop(&mut out, "ROLE", &c.role);
    }
    for p in &c.phones {
        out.push_str("TEL;TYPE=");
        out.push_str(&escape_param(&p.type_));
        out.push(':');
        out.push_str(&escape(&p.value));
        out.push_str("\r\n");
    }
    for e in &c.emails {
        out.push_str("EMAIL;TYPE=");
        out.push_str(&escape_param(&e.type_));
        out.push(':');
        out.push_str(&escape(&e.value));
        out.push_str("\r\n");
    }
    if !c.note.is_empty() {
        push_prop(&mut out, "NOTE", &c.note);
    }
    if !c.bday.is_empty() {
        push_prop(&mut out, "BDAY", &c.bday);
    }
    if !c.anniversary.is_empty() {
        push_prop(&mut out, "ANNIVERSARY", &c.anniversary);
    }
    for a in &c.addresses {
        out.push_str("ADR;TYPE=");
        out.push_str(&escape_param(&a.type_));
        out.push_str(":;;");
        out.push_str(&escape(&a.street));
        out.push(';');
        out.push_str(&escape(&a.city));
        out.push(';');
        out.push_str(&escape(&a.region));
        out.push(';');
        out.push_str(&escape(&a.postal));
        out.push(';');
        out.push_str(&escape(&a.country));
        out.push_str("\r\n");
    }
    for u in &c.urls {
        out.push_str("URL;TYPE=");
        out.push_str(&escape_param(&u.type_));
        out.push(':');
        out.push_str(&escape(&u.value));
        out.push_str("\r\n");
    }
    for u in &c.ims {
        out.push_str("IMPP;X-SERVICE-TYPE=");
        out.push_str(&escape_param(&u.type_));
        out.push(':');
        out.push_str(&escape(&u.value));
        out.push_str("\r\n");
    }
    for u in &c.socials {
        out.push_str("X-SOCIALPROFILE;TYPE=");
        out.push_str(&escape_param(&u.type_));
        out.push(':');
        out.push_str(&escape(&u.value));
        out.push_str("\r\n");
    }
    for u in &c.related {
        out.push_str("X-ABRELATEDNAMES;TYPE=");
        out.push_str(&escape_param(&u.type_));
        out.push(':');
        out.push_str(&escape(&u.value));
        out.push_str("\r\n");
    }
    for u in &c.dates {
        out.push_str("X-ABDATE;TYPE=");
        out.push_str(&escape_param(&u.type_));
        out.push(':');
        out.push_str(&escape(&u.value));
        out.push_str("\r\n");
    }
    if !c.groups.is_empty() {
        push_prop(&mut out, "CATEGORIES", &c.groups.join(","));
    }
    if c.is_group {
        push_prop(&mut out, "X-ADDRESSBOOKSERVER-KIND", "group");
        for m in &c.members {
            push_prop(&mut out, "X-ADDRESSBOOKSERVER-MEMBER", &format!("urn:uuid:{m}"));
        }
    }
    if let Some(photo) = &c.photo_jpeg {
        if photo.len() <= MAX_PHOTO_BYTES {
            let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, photo);
            out.push_str("PHOTO;ENCODING=b;TYPE=JPEG:");
            out.push_str(&fold_value(&b64));
            out.push_str("\r\n");
        }
    }
    let rev = if c.rev.is_empty() {
        now_rev()
    } else {
        c.rev.clone()
    };
    push_prop(&mut out, "REV", &rev);
    out.push_str("END:VCARD\r\n");
    out
}

pub fn now_rev() -> String {
    chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
}

pub fn rev_newer(a: &str, b: &str) -> bool {
    normalize_rev(a) > normalize_rev(b)
}

fn normalize_rev(r: &str) -> String {
    r.chars()
        .filter(|c| c.is_ascii_digit())
        .collect()
}

fn unfold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');
        if line.starts_with(' ') || line.starts_with('\t') {
            out.push_str(&line[1..]);
        } else {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
        }
    }
    out
}

fn split_prop(line: &str) -> Option<(String, String, String)> {
    let colon = line.find(':')?;
    let left = &line[..colon];
    let value = &line[colon + 1..];
    let (name, params) = match left.find(';') {
        Some(i) => (left[..i].to_string(), left[i + 1..].to_string()),
        None => (left.to_string(), String::new()),
    };
    Some((name, params, value.to_string()))
}

fn strip_group(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) => &name[i + 1..],
        None => name,
    }
}

fn split_grouped(name: &str) -> (String, String) {
    match name.find('.') {
        Some(i) => (name[..i].to_string(), name[i + 1..].to_string()),
        None => (String::new(), name.to_string()),
    }
}

fn remember_group(
    group: &str,
    field: GroupField,
    field_by_group: &mut HashMap<String, GroupField>,
    label_by_group: &HashMap<String, String>,
    c: &mut Contact,
) {
    if group.is_empty() {
        return;
    }
    field_by_group.insert(group.to_string(), field);
    if let Some(label) = label_by_group.get(group) {
        apply_group_label(c, field_by_group, group, label);
    }
}

fn apply_group_label(
    c: &mut Contact,
    field_by_group: &HashMap<String, GroupField>,
    group: &str,
    label: &str,
) {
    if label.is_empty() {
        return;
    }
    match field_by_group.get(group).copied() {
        Some(GroupField::Phone(i)) => {
            if let Some(p) = c.phones.get_mut(i) {
                p.type_ = label.to_string();
            }
        }
        Some(GroupField::Email(i)) => {
            if let Some(p) = c.emails.get_mut(i) {
                p.type_ = label.to_string();
            }
        }
        Some(GroupField::Url(i)) => {
            if let Some(p) = c.urls.get_mut(i) {
                p.type_ = label.to_string();
            }
        }
        Some(GroupField::Address(i)) => {
            if let Some(p) = c.addresses.get_mut(i) {
                p.type_ = label.to_string();
            }
        }
        Some(GroupField::Im(i)) => {
            if let Some(p) = c.ims.get_mut(i) {
                p.type_ = label.to_string();
            }
        }
        Some(GroupField::Social(i)) => {
            if let Some(p) = c.socials.get_mut(i) {
                p.type_ = label.to_string();
            }
        }
        Some(GroupField::Related(i)) => {
            if let Some(p) = c.related.get_mut(i) {
                p.type_ = label.to_string();
            }
        }
        None => {}
    }
}

fn decode_ab_label(v: &str) -> String {
    let v = v.trim();
    if let Some(inner) = v
        .strip_prefix("_$!<")
        .and_then(|s| s.strip_suffix(">!$_"))
    {
        return inner.to_ascii_lowercase();
    }
    v.to_string()
}

fn apply_org(c: &mut Contact, value: &str) {
    let parts: Vec<String> = unescape(value)
        .split(';')
        .map(|s| s.trim().to_string())
        .collect();
    c.org = parts.first().cloned().unwrap_or_default();
    c.department = parts.get(1).cloned().unwrap_or_default();
}

fn im_type(params: &str, value: &str) -> String {
    for part in params.split(';') {
        let p = part.trim();
        let upper = p.to_ascii_uppercase();
        if let Some(rest) = upper.strip_prefix("X-SERVICE-TYPE=") {
            let t = rest.trim().trim_matches('"').to_ascii_lowercase();
            if !t.is_empty() {
                return t;
            }
        }
    }
    let types = param_types(params);
    if let Some(t) = types.into_iter().find(|t| t != "pref" && t != "other") {
        return t;
    }
    if let Some((scheme, _)) = value.split_once(':') {
        let s = scheme.trim().to_ascii_lowercase();
        if !s.is_empty() && s != "http" && s != "https" {
            return s;
        }
    }
    "im".into()
}

fn strip_im_scheme(value: &str) -> String {
    let v = value.trim();
    match v.split_once(':') {
        Some((scheme, rest))
            if !scheme.contains('/') && !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") =>
        {
            rest.trim().to_string()
        }
        _ => v.to_string(),
    }
}

fn social_type(params: &str) -> String {
    let types = param_types(params);
    types
        .into_iter()
        .find(|t| t != "pref")
        .unwrap_or_else(|| "social".into())
}

fn related_type(params: &str) -> String {
    let types = param_types(params);
    types
        .into_iter()
        .find(|t| t != "pref")
        .unwrap_or_else(|| "related".into())
}

fn member_uid(v: &str) -> String {
    let v = v.trim();
    let v = v
        .strip_prefix("urn:uuid:")
        .or_else(|| v.strip_prefix("URN:UUID:"))
        .unwrap_or(v)
        .trim();
    v.trim_end_matches('/').to_string()
}

pub fn decode_markup(s: &str) -> String {
    let mut cur = s.to_string();
    for _ in 0..6 {
        let next = decode_markup_once(&cur);
        if next == cur {
            break;
        }
        cur = next;
    }
    cur
}

pub fn uid_core(uid: &str) -> String {
    let s = decode_markup(uid);
    let s = s
        .strip_prefix("urn:uuid:")
        .or_else(|| s.strip_prefix("URN:UUID:"))
        .unwrap_or(&s);
    let head = s.split('/').next().unwrap_or(s);
    head.chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

pub fn photo_uri(text: &str) -> Option<String> {
    let unfolded = unfold(&decode_markup(text));
    for line in unfolded.lines() {
        let Some((name, params, value)) = split_prop(line) else {
            continue;
        };
        if !strip_group(&name).eq_ignore_ascii_case("PHOTO") {
            continue;
        }
        let v = value.trim();
        let upper = params.to_ascii_uppercase();
        if v.starts_with("https://") || v.starts_with("http://") {
            return Some(v.to_string());
        }
        if upper.contains("VALUE=URI") && v.starts_with("https://") {
            return Some(v.to_string());
        }
    }
    None
}

fn decode_markup_once(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '&' {
            if c != '\r' {
                out.push(c);
            }
            continue;
        }
        let mut ent = String::from("&");
        while let Some(&n) = chars.peek() {
            chars.next();
            if n == '\\' {
                if matches!(chars.peek(), Some(';')) {
                    chars.next();
                    ent.push(';');
                    break;
                }
                ent.push(n);
                if ent.len() > 16 {
                    break;
                }
                continue;
            }
            ent.push(n);
            if n == ';' || ent.len() > 16 {
                break;
            }
        }
        if !ent.ends_with(';') {
            if let Some(ch) = numeric_entity(&format!("{ent};")) {
                if ch != '\r' {
                    out.push(ch);
                }
                continue;
            }
            out.push_str(&ent);
            continue;
        }
        let decoded = match ent.as_str() {
            "&amp;" => Some('&'),
            "&lt;" => Some('<'),
            "&gt;" => Some('>'),
            "&quot;" => Some('"'),
            "&apos;" => Some('\''),
            _ => numeric_entity(&ent),
        };
        if let Some(ch) = decoded {
            if ch != '\r' {
                out.push(ch);
            }
        } else {
            out.push_str(&ent);
        }
    }
    out
}

fn numeric_entity(ent: &str) -> Option<char> {
    let body = ent
        .trim_start_matches('&')
        .trim_end_matches(';')
        .replace('\\', "");
    let num = if let Some(hex) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
        u32::from_str_radix(hex, 16).ok()?
    } else if let Some(dec) = body.strip_prefix('#') {
        dec.parse::<u32>().ok()?
    } else {
        return None;
    };
    char::from_u32(num)
}

fn unescape(v: &str) -> String {
    let mut out = String::new();
    let mut chars = v.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') | Some('N') => out.push('\n'),
                Some(',') => out.push(','),
                Some(';') => out.push(';'),
                Some('\\') => out.push('\\'),
                Some(other) => out.push(other),
                None => {}
            }
        } else if c != '\r' {
            out.push(c);
        }
    }
    out.trim().to_string()
}

fn normalize_tel(v: &str) -> String {
    let v = v.trim();
    let v = v
        .strip_prefix("tel:")
        .or_else(|| v.strip_prefix("TEL:"))
        .unwrap_or(v)
        .trim();
    v.trim_end_matches(['\\', ';']).trim().to_string()
}

fn escape(v: &str) -> String {
    v.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

fn escape_param(v: &str) -> String {
    v.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect()
}

fn push_prop(out: &mut String, name: &str, value: &str) {
    out.push_str(name);
    out.push(':');
    out.push_str(&escape(value));
    out.push_str("\r\n");
}

fn fold_value(v: &str) -> String {
    // Keep PHOTO on one logical line; unfolding already handles wraps on read.
    v.chars().filter(|c| !c.is_whitespace()).collect()
}

fn apply_n(c: &mut Contact, value: &str) {
    let parts: Vec<String> = value.split(';').map(unescape).collect();
    c.last = parts.first().cloned().unwrap_or_default();
    c.first = parts.get(1).cloned().unwrap_or_default();
    c.middle = parts.get(2).cloned().unwrap_or_default();
    c.prefix = parts.get(3).cloned().unwrap_or_default();
    c.suffix = parts.get(4).cloned().unwrap_or_default();
}

fn param_types(params: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in params.split(';') {
        let p = part.trim();
        let upper = p.to_ascii_uppercase();
        if let Some(rest) = upper.strip_prefix("TYPE=") {
            for t in rest.split(',') {
                let t = t.trim().trim_matches('"');
                if !t.is_empty() {
                    out.push(t.to_ascii_lowercase());
                }
            }
        } else if !p.is_empty() && !upper.contains('=') {
            out.push(p.to_ascii_lowercase());
        }
    }
    out
}

fn tel_type(params: &str) -> String {
    let types = param_types(params);
    for want in ["cell", "mobile", "iphone", "work", "home", "fax", "main", "pager", "voice", "pref"] {
        if types.iter().any(|t| t == want) {
            return if want == "mobile" {
                "cell".into()
            } else {
                want.into()
            };
        }
    }
    "other".into()
}

fn email_type(params: &str) -> String {
    let types = param_types(params);
    for want in ["work", "home", "internet", "pref"] {
        if types.iter().any(|t| t == want) {
            return if want == "internet" || want == "pref" {
                "other".into()
            } else {
                want.into()
            };
        }
    }
    "other".into()
}

fn url_type(params: &str) -> String {
    let types = param_types(params);
    for want in ["work", "home", "pref"] {
        if types.iter().any(|t| t == want) {
            return if want == "pref" {
                "other".into()
            } else {
                want.into()
            };
        }
    }
    "other".into()
}

fn parse_adr(params: &str, value: &str) -> Option<Address> {
    let parts: Vec<String> = value.split(';').map(unescape).collect();
    let street = [parts.get(1), parts.get(2)]
        .into_iter()
        .flatten()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let addr = Address {
        type_: {
            let types = param_types(params);
            if types.iter().any(|t| t == "work") {
                "work".into()
            } else if types.iter().any(|t| t == "home") {
                "home".into()
            } else {
                "other".into()
            }
        },
        street,
        city: parts.get(3).cloned().unwrap_or_default(),
        region: parts.get(4).cloned().unwrap_or_default(),
        postal: parts.get(5).cloned().unwrap_or_default(),
        country: parts.get(6).cloned().unwrap_or_default(),
    };
    if addr.street.is_empty()
        && addr.city.is_empty()
        && addr.region.is_empty()
        && addr.postal.is_empty()
        && addr.country.is_empty()
    {
        None
    } else {
        Some(addr)
    }
}

fn decode_note(params: &str, value: &str) -> String {
    let enc = params.to_ascii_uppercase();
    if enc.contains("QUOTED-PRINTABLE") {
        unescape(&decode_qp(value))
    } else {
        unescape(value)
    }
}

fn decode_qp(v: &str) -> String {
    let mut bytes = Vec::new();
    let compact: String = v.chars().filter(|c| *c != '\n' && *c != '\r').collect();
    let b = compact.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'=' && i + 2 < b.len() {
            if let Ok(n) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16)
            {
                bytes.push(n);
                i += 3;
                continue;
            }
        }
        bytes.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn parse_photo(params: &str, value: &str) -> Option<Vec<u8>> {
    let p = params.to_ascii_uppercase();
    let v = value.trim();
    let b64 = if let Some(rest) = v.strip_prefix("data:image/jpeg;base64,") {
        rest
    } else if let Some(rest) = v.strip_prefix("data:image/jpg;base64,") {
        rest
    } else if p.contains("ENCODING=B")
        || p.contains("ENCODING=BASE64")
        || p.contains("TYPE=JPEG")
        || p.contains("TYPE=JPG")
    {
        if v.starts_with("http://") || v.starts_with("https://") {
            return None;
        }
        v
    } else if v.starts_with("/9j/") {
        v
    } else {
        return None;
    };
    let compact: String = b64.chars().filter(|c| !c.is_whitespace()).collect();
    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &compact).ok()?;
    if decoded.len() > MAX_PHOTO_BYTES || decoded.len() < 32 {
        return None;
    }
    if decoded[0] != 0xFF || decoded[1] != 0xD8 {
        return None;
    }
    Some(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_multi_card() {
        let src = "BEGIN:VCARD\r\nVERSION:3.0\r\nN:Last;First;;;\r\nFN:First Last\r\nTEL;type=CELL:+1555\r\nEMAIL;type=INTERNET;type=WORK:a@b.com\r\nUID:one\r\nEND:VCARD\r\nBEGIN:VCARD\r\nVERSION:3.0\r\nitem1.EMAIL;type=INTERNET:home@b.com\r\nitem1.X-ABLabel:_$!<Home>!$_\r\nN:Two;Person;;;\r\nFN:Person Two\r\nNICKNAME:Pat\r\nORG:Acme\r\nTITLE:Eng\r\nUID:two\r\nEND:VCARD\r\n";
        let cards = split_cards(src);
        assert_eq!(cards.len(), 2);
        let a = parse_card(&cards[0]).unwrap();
        assert_eq!(a.first, "First");
        assert_eq!(a.phones[0].value, "+1555");
        assert_eq!(a.emails[0].type_, "work");
        let b = parse_card(&cards[1]).unwrap();
        assert_eq!(b.nickname, "Pat");
        assert_eq!(b.emails[0].value, "home@b.com");
        assert_eq!(b.org, "Acme");
    }

    #[test]
    fn folded_photo_roundtrip() {
        let jpeg = {
            let mut v = vec![0xFF, 0xD8];
            v.extend(std::iter::repeat(0x11).take(64));
            v.extend([0xFF, 0xD9]);
            v
        };
        let mut c = Contact {
            uid: "p1".into(),
            first: "Ada".into(),
            last: "Lovelace".into(),
            photo_jpeg: Some(jpeg.clone()),
            ..Contact::default()
        };
        c.normalize();
        let text = serialize_card(&c);
        assert!(text.contains("PHOTO;ENCODING=b;TYPE=JPEG:"));
        let back = parse_card(&text).unwrap();
        assert_eq!(back.photo_jpeg.as_ref().unwrap().len(), jpeg.len());
        assert_eq!(back.first, "Ada");
    }

    #[test]
    fn icloud_xml_entities_and_tel_uri() {
        let src = "BEGIN:VCARD\r\nVERSION:3.0\r\nN:Amp;Tom;;;\r\nFN:Tom & Jerry\r\nTEL;TYPE=CELL:tel:+61411112222\r\nUID:amp1\r\nEND:VCARD\r\n";
        let c = parse_card(src).unwrap();
        assert_eq!(c.fn_, "Tom & Jerry");
        assert_eq!(c.phones[0].value, "+61411112222");
        let leftover = parse_card(
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Ada\r\nTEL;TYPE=CELL:+61411112222\\;\r\nUID:amp2\r\nEND:VCARD\r\n",
        )
        .unwrap();
        assert_eq!(leftover.phones[0].value, "+61411112222");
        let extra = parse_card(
            "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:n1\r\nFN:Ada\r\nNOTE:Water heating inspection guy\r\nADR;TYPE=HOME:;;10 Main St;Melbourne;VIC;3000;Australia\r\nURL;TYPE=WORK:https://example.com\r\nBDAY:1990-01-02\r\nTEL;TYPE=CELL:0411\r\nTEL;TYPE=WORK:0399\r\nEND:VCARD\r\n",
        )
        .unwrap();
        assert_eq!(extra.note, "Water heating inspection guy");
        assert_eq!(extra.phones.len(), 2);
        assert_eq!(extra.addresses[0].city, "Melbourne");
        assert_eq!(extra.urls[0].value, "https://example.com");
        assert_eq!(extra.bday, "1990-01-02");
        let group = parse_card(
            "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:g1\r\nFN:Family\r\nX-ADDRESSBOOKSERVER-KIND:group\r\nX-ADDRESSBOOKSERVER-MEMBER:urn:uuid:6F5812EB-AA85-4B89-AACC-D59A07D4F262\r\nEND:VCARD\r\n",
        )
        .unwrap();
        assert!(group.is_group);
        assert_eq!(group.fn_, "Family");
        assert_eq!(group.members[0], "6F5812EB-AA85-4B89-AACC-D59A07D4F262");
        let apple = parse_card(
            "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:6F5812EB-AA85-4B89-AACC-D59A07D4F262&#13\\;\r\nFN:Tom &amp; Jerry&#13\\;\r\nTEL;TYPE=other:+61 431 034 583&#13\\;\r\nREV:2024-01-14T13:55:36Z&#13\\;\r\nEND:VCARD\r\n",
        )
        .unwrap();
        assert_eq!(apple.uid, "6F5812EB-AA85-4B89-AACC-D59A07D4F262");
        assert_eq!(apple.fn_, "Tom & Jerry");
        assert_eq!(apple.phones[0].value, "+61 431 034 583");
        assert!(!apple.uid.contains('&'));
        assert!(!apple.phones[0].value.contains('&'));
        assert_eq!(
            uid_core("6F5812EB-AA85-4B89-AACC-D59A07D4F262&#13;"),
            uid_core("6F5812EB-AA85-4B89-AACC-D59A07D4F262/ABPerson")
        );
    }

    #[test]
    fn rev_last_write_wins() {
        assert!(rev_newer("20240102T000000Z", "20240101T000000Z"));
        assert!(!rev_newer("2024-01-01", "20240102T00"));
    }

    #[test]
    fn apple_extra_fields() {
        let src = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:marie\r\nN:Curie;Marie;Skłodowska;Dr.;PhD\r\nFN:Dr. Marie Curie\r\nORG:Radium Institute;Physics\r\nTITLE:Professor\r\nROLE:Researcher\r\nitem1.TEL;type=CELL:+331\r\nitem1.X-ABLabel:Lab\r\nitem2.EMAIL;type=INTERNET:m@lab.fr\r\nitem2.X-ABLabel:_$!<Work>!$_\r\nIMPP;X-SERVICE-TYPE=Skype:skype:marie.curie\r\nX-SOCIALPROFILE;TYPE=twitter:https://twitter.com/marie\r\nitem3.X-ABRELATEDNAMES:Pierre Curie\r\nitem3.X-ABLabel:_$!<Spouse>!$_\r\nitem4.X-ABDATE:1895-07-26\r\nitem4.X-ABLabel:_$!<Anniversary>!$_\r\nX-JABBER:marie@chat\r\nEND:VCARD\r\n";
        let c = parse_card(src).unwrap();
        assert_eq!(c.middle, "Skłodowska");
        assert_eq!(c.prefix, "Dr.");
        assert_eq!(c.suffix, "PhD");
        assert_eq!(c.department, "Physics");
        assert_eq!(c.role, "Researcher");
        assert_eq!(c.phones[0].type_, "Lab");
        assert_eq!(c.emails[0].type_, "work");
        assert_eq!(c.ims[0].type_, "skype");
        assert_eq!(c.ims[0].value, "marie.curie");
        assert_eq!(c.ims[1].type_, "jabber");
        assert_eq!(c.socials[0].type_, "twitter");
        assert_eq!(c.related[0].type_, "spouse");
        assert_eq!(c.related[0].value, "Pierre Curie");
        assert_eq!(c.anniversary, "1895-07-26");
        let back = parse_card(&serialize_card(&c)).unwrap();
        assert_eq!(back.department, "Physics");
        assert_eq!(back.ims.len(), 2);
        assert_eq!(back.socials[0].value, "https://twitter.com/marie");
    }
}
