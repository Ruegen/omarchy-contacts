use std::io::{self, Read};
use std::time::Duration;

use ureq::http::{Method, Request};
use ureq::{Agent, Body, Error as UreqError};

use crate::contact::Contact;
use crate::vcard::{parse_card, serialize_card, split_cards};

pub const MAX_BODY: usize = 2 * 1024 * 1024;
pub const MAX_PROPFIND: usize = 8 * 1024 * 1024;
const MAX_REDIRECTS: u32 = 5;
const PER_REQ_SECS: u64 = 60;

#[derive(Clone, Debug)]
pub struct CardDavClient {
    agent: Agent,
    apple_id: String,
    password: String,
}

#[derive(Clone, Debug)]
pub struct RemoteCard {
    pub href: String,
    pub etag: String,
    pub contact: Contact,
}

impl CardDavClient {
    pub fn new(apple_id: String, password: String) -> Self {
        let agent = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(PER_REQ_SECS)))
            .max_redirects(MAX_REDIRECTS)
            .http_status_as_error(false)
            .build()
            .new_agent();
        Self {
            agent,
            apple_id,
            password,
        }
    }

    fn basic(&self) -> String {
        let raw = format!("{}:{}", self.apple_id, self.password);
        format!(
            "Basic {}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, raw.as_bytes())
        )
    }

    pub fn request(
        &self,
        method: &str,
        url: &str,
        depth: Option<&str>,
        content_type: Option<&str>,
        body: Option<&str>,
        extra: &[(&str, &str)],
    ) -> io::Result<(u16, String, Vec<u8>)> {
        if !url_allowed(url) {
            return Err(io::Error::other("refusing non-iCloud URL"));
        }
        let m = Method::from_bytes(method.as_bytes()).map_err(|_| io::Error::other("bad method"))?;
        let mut b = Request::builder().method(m).uri(url);
        b = b.header("Authorization", self.basic());
        b = b.header("User-Agent", "omarchy-contacts/0.2");
        if let Some(d) = depth {
            b = b.header("Depth", d);
        }
        if let Some(ct) = content_type {
            b = b.header("Content-Type", ct);
        }
        for (k, v) in extra {
            b = b.header(*k, *v);
        }
        let req = if let Some(body) = body {
            b.body(body.to_string())
                .map_err(|e| io::Error::other(e.to_string()))?
        } else {
            b.body(String::new())
                .map_err(|e| io::Error::other(e.to_string()))?
        };
        match self.agent.run(req) {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let etag = resp
                    .headers()
                    .get("ETag")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                let loc = resp
                    .headers()
                    .get("Location")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                let body = read_body(resp.into_body(), MAX_BODY)?;
                if (300..400).contains(&status) && !loc.is_empty() {
                    return Ok((status, loc, body));
                }
                Ok((status, etag, body))
            }
            Err(UreqError::StatusCode(code)) => Err(io::Error::other(format!("http {code}"))),
            Err(e) => Err(io::Error::other(e.to_string())),
        }
    }

    pub fn propfind(&self, url: &str, depth: &str, xml: &str) -> io::Result<(u16, String, Vec<u8>)> {
        let mut res = self.request(
            "PROPFIND",
            url,
            Some(depth),
            Some("application/xml; charset=utf-8"),
            Some(xml),
            &[],
        )?;
        if res.2.len() > MAX_PROPFIND {
            res.2.truncate(MAX_PROPFIND);
        }
        Ok(res)
    }

    pub fn put_card(&self, url: &str, contact: &Contact, etag: Option<&str>) -> io::Result<String> {
        let body = serialize_card(contact);
        let extra = if let Some(t) = etag {
            vec![("If-Match", t)]
        } else {
            vec![]
        };
        let (status, etag, _) = self.request(
            "PUT",
            url,
            None,
            Some("text/vcard; charset=utf-8"),
            Some(&body),
            &extra,
        )?;
        if status == 201 || status == 204 || status == 200 {
            Ok(etag)
        } else {
            Err(io::Error::other(format!("put {status}")))
        }
    }

    pub fn get_card(&self, url: &str) -> io::Result<RemoteCard> {
        let (status, etag, body) = self.request("GET", url, None, None, None, &[])?;
        if status != 200 {
            return Err(io::Error::other(format!("get {status}")));
        }
        let text = String::from_utf8_lossy(&body);
        let card = split_cards(&text)
            .into_iter()
            .next()
            .ok_or_else(|| io::Error::other("empty vcard"))?;
        let contact = parse_card(&card).map_err(|e| io::Error::other(e.0))?;
        Ok(RemoteCard {
            href: url.to_string(),
            etag,
            contact,
        })
    }

    pub fn delete(&self, url: &str, etag: Option<&str>) -> io::Result<()> {
        let extra = if let Some(t) = etag {
            vec![("If-Match", t)]
        } else {
            vec![]
        };
        let (status, _, _) = self.request("DELETE", url, None, None, None, &extra)?;
        if status == 204 || status == 200 || status == 404 {
            Ok(())
        } else {
            Err(io::Error::other(format!("delete {status}")))
        }
    }
}

fn read_body(mut body: Body, max: usize) -> io::Result<Vec<u8>> {
    let mut r = body.as_reader();
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    loop {
        let n = r.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        if buf.len().saturating_add(n) > max {
            return Err(io::Error::other("response too large"));
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    Ok(buf)
}

pub fn url_allowed(url: &str) -> bool {
    let Ok(u) = url::parse_ish(url) else {
        return false;
    };
    u
}

/// Tiny https URL check without a url crate: scheme + host suffix only.
mod url {
    pub fn parse_ish(url: &str) -> Result<bool, ()> {
        let url = url.trim();
        if url.len() > 2048 || url.contains('\n') || url.contains('\r') {
            return Err(());
        }
        let rest = url.strip_prefix("https://").ok_or(())?;
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        let host = host.split('@').next_back().unwrap_or("");
        let host = host.split(':').next().unwrap_or("").to_ascii_lowercase();
        if host == "icloud.com"
            || host.ends_with(".icloud.com")
            || host == "apple.com"
            || host.ends_with(".apple.com")
        {
            Ok(true)
        } else {
            Err(())
        }
    }
}

pub fn xml_hrefs(xml: &str) -> Vec<String> {
    xml_tag_values(xml, "href")
}

pub fn xml_tag_values(xml: &str, tag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = xml.to_ascii_lowercase();
    let mut i = 0;
    while i < lower.len() {
        let rel = match find_tag(&lower, i, tag) {
            Some(p) => p,
            None => break,
        };
        let start = rel.end;
        if let Some(end) = find_close(&lower, start, tag) {
            let raw = xml.get(start..end).unwrap_or("");
            let v = decode_xml(raw.trim());
            if !v.is_empty() {
                out.push(v);
            }
            i = end + 2;
        } else {
            i = rel.end + 1;
        }
    }
    out
}

struct Span {
    end: usize,
}

fn find_tag(lower: &str, from: usize, tag: &str) -> Option<Span> {
    let needle = format!("{tag}>");
    let mut s = from;
    while let Some(p) = lower[s..].find(&needle) {
        let abs = s + p;
        // require '<' or ':' just before the tag name
        if abs == 0 {
            s = abs + 1;
            continue;
        }
        let before = &lower[..abs];
        if before.ends_with('<') || before.ends_with(':') {
            return Some(Span {
                end: abs + needle.len(),
            });
        }
        s = abs + 1;
    }
    None
}

fn find_close(lower: &str, from: usize, tag: &str) -> Option<usize> {
    let mut s = from;
    while let Some(p) = lower[s..].find("</") {
        let abs = s + p;
        let rest = &lower[abs + 2..];
        let name = rest.split('>').next().unwrap_or("");
        let local = name.rsplit(':').next().unwrap_or("");
        if local == tag {
            return Some(abs);
        }
        s = abs + 2;
    }
    None
}

fn decode_xml(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

pub fn join_url(base: &str, href: &str) -> String {
    if href.starts_with("https://") {
        href.to_string()
    } else if href.starts_with('/') {
        if let Some(scheme) = base.find("://") {
            let after = &base[scheme + 3..];
            let host = after.split('/').next().unwrap_or("");
            format!("https://{host}{href}")
        } else {
            href.to_string()
        }
    } else {
        let base = base.trim_end_matches('/');
        format!("{base}/{href}")
    }
}

pub const CURRENT_USER_PRINCIPAL: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:">
  <d:prop><d:current-user-principal/></d:prop>
</d:propfind>"#;

pub const ADDRESSBOOK_HOME: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
  <d:prop><c:addressbook-home-set/></d:prop>
</d:propfind>"#;

pub const ADDRESSBOOK_LIST: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
  <d:prop>
    <d:resourcetype/>
    <d:displayname/>
    <c:addressbook-description/>
  </d:prop>
</d:propfind>"#;

pub const ADDRESSBOOK_INDEX: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
  <d:prop>
    <d:getetag/>
    <d:getcontenttype/>
  </d:prop>
</d:propfind>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_icloud_hosts() {
        assert!(url_allowed("https://contacts.icloud.com/.well-known/carddav"));
        assert!(url_allowed("https://p123-contacts.icloud.com/co/"));
        assert!(!url_allowed("http://contacts.icloud.com/"));
        assert!(!url_allowed("https://evil.example/icloud.com"));
        assert!(!url_allowed("https://icloud.com.evil.example/"));
    }

    #[test]
    fn href_extract() {
        let xml = r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/principal/foo/</d:href></d:response></d:multistatus>"#;
        let hs = xml_hrefs(xml);
        assert_eq!(hs[0], "/principal/foo/");
    }
}
