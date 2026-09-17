//! vCard 3.0 / 4.0 subset used by the address book.

use crate::contact::{Address, Contact, Email, Link, Phone};
use crate::paths::MAX_PHOTO_BYTES;

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
    for line in unfolded.lines() {
        if line.is_empty() {
            continue;
        }
        let upper = line.to_ascii_uppercase();
        if upper == "BEGIN:VCARD" || upper == "END:VCARD" {
            continue;
        }
        let Some((name, params, value)) = split_prop(line) else {
            continue;
        };
        let name = strip_group(&name).to_ascii_uppercase();
        match name.as_str() {
            "VERSION" => {}
            "UID" => c.uid = unescape(&value),
            "FN" => c.fn_ = unescape(&value),
            "N" => apply_n(&mut c, &value),
            "NICKNAME" => c.nickname = unescape(&value),
            "ORG" => c.org = unescape(&value).split(';').next().unwrap_or("").to_string(),
            "TITLE" => c.title = unescape(&value),
            "NOTE" => c.note = decode_note(&params, &value),
            "BDAY" => c.bday = unescape(&value),
            "REV" => c.rev = unescape(&value),
            "TEL" => {
                let v = normalize_tel(&unescape(&value));
                if !v.is_empty() {
                    c.phones.push(Phone {
                        type_: tel_type(&params),
                        value: v,
                    });
                }
            }
            "EMAIL" => {
                let v = unescape(&value);
                if !v.is_empty() {
                    c.emails.push(Email {
                        type_: email_type(&params),
                        value: v,
                    });
                }
            }
            "ADR" => {
                if let Some(addr) = parse_adr(&params, &value) {
                    c.addresses.push(addr);
                }
            }
            "URL" => {
                let v = unescape(&value);
                if !v.is_empty() {
                    c.urls.push(Link {
                        type_: url_type(&params),
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
    let n = format!("{};{};;;", escape(&c.last), escape(&c.first));
    out.push_str("N:");
    out.push_str(&n);
    out.push_str("\r\n");
    if !c.nickname.is_empty() {
        push_prop(&mut out, "NICKNAME", &c.nickname);
    }
    if !c.org.is_empty() {
        push_prop(&mut out, "ORG", &c.org);
    }
    if !c.title.is_empty() {
        push_prop(&mut out, "TITLE", &c.title);
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
    for want in ["cell", "mobile", "work", "home", "fax", "voice", "pref"] {
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
}
