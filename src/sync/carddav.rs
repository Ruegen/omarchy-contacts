use std::io;
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::header::HeaderValue;
use reqwest::redirect::Policy;
use reqwest::Method;

use crate::contact::Contact;
use crate::vcard::{decode_markup, parse_card, photo_uri, serialize_card, split_cards};

pub const MAX_BODY: usize = 2 * 1024 * 1024;
pub const MAX_PROPFIND: usize = 32 * 1024 * 1024;
const MAX_REDIRECTS: u32 = 8;
const PER_REQ_SECS: u64 = 60;

#[derive(Clone, Debug)]
pub struct DavResponse {
    pub status: u16,
    pub etag: String,
    pub body: Vec<u8>,
    pub url: String,
    pub partition: String,
    pub instance: String,
}

#[derive(Clone, Debug)]
pub struct CardDavClient {
    client: Client,
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
        let client = Client::builder()
            .timeout(Duration::from_secs(PER_REQ_SECS))
            .connect_timeout(Duration::from_secs(20))
            .redirect(Policy::none())
            .https_only(true)
            .build()
            .expect("https client");
        Self {
            client,
            apple_id,
            password,
        }
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
        let r = self.send(method, url, depth, content_type, body, extra)?;
        if r.status == 401 || r.status == 403 {
            return Err(io::Error::other(
                "iCloud refused the sign-in. Check the Apple ID and app-specific password.",
            ));
        }
        Ok((r.status, r.etag, r.body))
    }

    pub fn propfind_at(&self, url: &str, depth: &str, xml: &str) -> io::Result<DavResponse> {
        self.send(
            "PROPFIND",
            url,
            Some(depth),
            Some("application/xml; charset=utf-8"),
            Some(xml),
            &[],
        )
    }

    pub fn report_at(&self, url: &str, xml: &str) -> io::Result<DavResponse> {
        self.send(
            "REPORT",
            url,
            Some("1"),
            Some("application/xml; charset=utf-8"),
            Some(xml),
            &[],
        )
    }

    fn send(
        &self,
        method: &str,
        url: &str,
        depth: Option<&str>,
        content_type: Option<&str>,
        body: Option<&str>,
        extra: &[(&str, &str)],
    ) -> io::Result<DavResponse> {
        let m = Method::from_bytes(method.as_bytes()).map_err(|_| io::Error::other("bad method"))?;
        let mut current = url.trim().to_string();
        for _ in 0..MAX_REDIRECTS {
            if !url_allowed(&current) {
                return Err(io::Error::other("refusing non-iCloud URL"));
            }
            let mut b = self
                .client
                .request(m.clone(), &current)
                .header("User-Agent", "omarchy-contacts/0.2")
                .basic_auth(&self.apple_id, Some(&self.password));
            if let Some(d) = depth {
                b = b.header("Depth", d);
            }
            if let Some(ct) = content_type {
                b = b.header("Content-Type", ct);
            }
            for (k, v) in extra {
                let val = HeaderValue::from_str(v).map_err(|e| io::Error::other(e.to_string()))?;
                b = b.header(*k, val);
            }
            if let Some(body) = body {
                b = b.body(body.to_string());
            }
            let resp = b.send().map_err(|e| io::Error::other(http_err(&e)))?;
            let status = resp.status().as_u16();
            let etag = header_text(resp.headers().get("etag"));
            let loc = header_text(resp.headers().get("location"));
            let partition = header_text(resp.headers().get("x-apple-user-partition"));
            let instance = header_text(resp.headers().get("x-responding-instance"));
            let bytes = resp.bytes().map_err(|e| io::Error::other(http_err(&e)))?;
            let cap = if m.as_str() == "PROPFIND" || m.as_str() == "REPORT" {
                MAX_PROPFIND
            } else {
                MAX_BODY
            };
            if bytes.len() > cap {
                return Err(io::Error::other("response too large"));
            }
            if (300..400).contains(&status) {
                if loc.is_empty() {
                    return Err(io::Error::other(format!("iCloud redirected ({status}) with no new address")));
                }
                current = join_url(&current, &loc);
                continue;
            }
            return Ok(DavResponse {
                status,
                etag,
                body: bytes.to_vec(),
                url: current,
                partition,
                instance,
            });
        }
        Err(io::Error::other("too many iCloud redirects"))
    }

    pub fn propfind(&self, url: &str, depth: &str, xml: &str) -> io::Result<(u16, String, Vec<u8>)> {
        let mut res = self.propfind_at(url, depth, xml)?;
        if res.body.len() > MAX_PROPFIND {
            res.body.truncate(MAX_PROPFIND);
        }
        Ok((res.status, res.etag, res.body))
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
        let mut contact = parse_card(&card).map_err(|e| io::Error::other(e.0))?;
        if contact.photo_jpeg.is_none() {
            if let Some(uri) = photo_uri(&card) {
                if let Ok(bytes) = self.get_bytes(&uri) {
                    if bytes.len() >= 32
                        && bytes.len() <= crate::paths::MAX_PHOTO_BYTES
                        && bytes[0] == 0xFF
                        && bytes[1] == 0xD8
                    {
                        contact.photo_jpeg = Some(bytes);
                        contact.has_photo = true;
                    }
                }
            }
        }
        Ok(RemoteCard {
            href: url.to_string(),
            etag,
            contact,
        })
    }

    pub fn get_bytes(&self, url: &str) -> io::Result<Vec<u8>> {
        let (status, _, body) = self.request("GET", url, None, None, None, &[])?;
        if status != 200 {
            return Err(io::Error::other(format!("get {status}")));
        }
        Ok(body)
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

fn header_text(v: Option<&HeaderValue>) -> String {
    v.and_then(|h| h.to_str().ok()).unwrap_or("").to_string()
}

fn http_err(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "iCloud took too long to answer".into()
    } else if e.is_connect() {
        "Could not reach iCloud".into()
    } else {
        "Could not talk to iCloud".into()
    }
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
            let v = decode_markup(raw.trim());
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
    let mut s = from;
    while let Some(p) = lower[s..].find(tag) {
        let abs = s + p;
        if abs == 0 {
            s = abs + 1;
            continue;
        }
        let before = lower.as_bytes()[abs - 1] as char;
        if before != '<' && before != ':' {
            s = abs + 1;
            continue;
        }
        let after_i = abs + tag.len();
        let after = lower.get(after_i..).unwrap_or("");
        let first = after.chars().next();
        let ok = matches!(first, Some('>' | ' ' | '\t' | '\n' | '\r' | '/'));
        if !ok {
            s = abs + 1;
            continue;
        }
        let Some(gt) = after.find('>') else {
            return None;
        };
        let open = &after[..gt];
        if open.trim_end().ends_with('/') {
            s = after_i + gt + 1;
            continue;
        }
        return Some(Span {
            end: after_i + gt + 1,
        });
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

/// First href nested inside `<prop>…</prop>`.
pub fn href_in_prop(xml: &str, prop: &str) -> Option<String> {
    let lower = xml.to_ascii_lowercase();
    let open = find_tag(&lower, 0, prop)?;
    let close = find_close(&lower, open.end, prop)?;
    xml_hrefs(&xml[open.end..close]).into_iter().next()
}

pub fn pick_principal(xml: &str) -> Option<String> {
    if let Some(h) = href_in_prop(xml, "current-user-principal") {
        if !h.to_ascii_lowercase().contains(".well-known") {
            return Some(h);
        }
    }
    xml_hrefs(xml).into_iter().find(|h| {
        let l = h.to_ascii_lowercase();
        (l.contains("/principal") || l.contains("/principals/")) && !l.contains(".well-known")
    })
}

pub fn contacts_host_from_xml(xml: &str) -> Option<String> {
    for h in xml_hrefs(xml) {
        let rest = h.strip_prefix("https://").unwrap_or("");
        if rest.is_empty() {
            continue;
        }
        let hostport = rest.split(['/', '?', '#']).next().unwrap_or("");
        let host = hostport.split(':').next().unwrap_or("").to_ascii_lowercase();
        if host.ends_with("-contacts.icloud.com") {
            return Some(host);
        }
    }
    None
}

pub fn xml_shape(xml: &str) -> String {
    let t = xml.trim_start();
    let xmlish = t.starts_with('<') || t.to_ascii_lowercase().contains("multistatus");
    let gzip = xml.as_bytes().starts_with(&[0x1f, 0x8b]);
    format!("bytes={} xml={} gzip={}", xml.len(), xmlish, gzip)
}

fn response_chunks(xml: &str) -> Vec<String> {
    let lower = xml.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(open) = find_tag(&lower, i, "response") {
        match find_close(&lower, open.end, "response") {
            Some(end) => {
                out.push(xml[open.end..end].to_string());
                i = end + 2;
            }
            None => break,
        }
    }
    out
}

fn resourcetype_is_addressbook(chunk: &str) -> bool {
    let lower = chunk.to_ascii_lowercase();
    let Some(open) = find_tag(&lower, 0, "resourcetype") else {
        return false;
    };
    let Some(end) = find_close(&lower, open.end, "resourcetype") else {
        return false;
    };
    let rt = &lower[open.end..end];
    rt.contains("addressbook") && !rt.contains("addressbook-home-set")
}

pub fn addressbook_hrefs(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in response_chunks(xml) {
        if resourcetype_is_addressbook(&chunk) {
            if let Some(h) = xml_hrefs(&chunk).into_iter().next() {
                out.push(h);
            }
        }
    }
    out
}

pub fn card_hrefs(xml: &str, book: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in response_chunks(xml) {
        if resourcetype_is_addressbook(&chunk) {
            continue;
        }
        for h in xml_hrefs(&chunk) {
            let l = h.to_ascii_lowercase();
            let joined = join_url(book, &h);
            if joined == *book || joined.trim_end_matches('/') == book.trim_end_matches('/') {
                continue;
            }
            if l.ends_with('/') {
                continue;
            }
            out.push(h);
        }
    }
    if out.is_empty() {
        for h in xml_hrefs(xml) {
            let l = h.to_ascii_lowercase();
            if l.ends_with(".vcf") || l.contains("/card/") && !l.ends_with('/') {
                out.push(h);
            }
        }
    }
    out
}

pub fn vcards_in_multistatus(xml: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for chunk in response_chunks(xml) {
        let href = xml_hrefs(&chunk).into_iter().next().unwrap_or_default();
        for data in xml_tag_values(&chunk, "address-data") {
            // xml_tag_values already unescapes XML entities.
            if data.to_ascii_uppercase().contains("BEGIN:VCARD") {
                out.push((href.clone(), data));
            }
        }
    }
    out
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

/// iCloud’s front door often 401s and points at a numbered contacts host.
pub fn icloud_contacts_host(partition: &str, instance: &str) -> Option<String> {
    let p = partition.trim();
    if !p.is_empty() && p.len() <= 8 && p.chars().all(|c| c.is_ascii_digit()) {
        return Some(format!("p{p}-contacts.icloud.com"));
    }
    let hay = instance.to_ascii_lowercase();
    if let Some(idx) = hay.find("-carddav") {
        let before = &hay[..idx];
        let start = before
            .rfind(|c: char| !c.is_ascii_alphanumeric())
            .map(|i| i + 1)
            .unwrap_or(0);
        let prefix = &before[start..];
        if prefix.starts_with('p')
            && prefix.len() > 1
            && prefix[1..].chars().all(|c| c.is_ascii_digit())
        {
            return Some(format!("{prefix}-contacts.icloud.com"));
        }
    }
    None
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

pub const ADDRESSBOOK_QUERY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<c:addressbook-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
  <d:prop>
    <d:getetag/>
    <c:address-data/>
  </d:prop>
</c:addressbook-query>"#;

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

    #[test]
    fn xml_numeric_entities() {
        let xml = "<href>Tom &amp; Jerry&#13;</href>";
        assert_eq!(xml_hrefs(xml)[0], "Tom & Jerry");
        let xml = "<href>&#43;61411112222</href>";
        assert_eq!(xml_hrefs(xml)[0], "+61411112222");
        let xml = r#"<response><href>/a.vcf</href><address-data>BEGIN:VCARD
FN:Tom &amp; Jerry&#13;
TEL:&#43;61411112222
UID:1
END:VCARD</address-data></response>"#;
        let cards = vcards_in_multistatus(xml);
        assert!(cards[0].1.contains("Tom & Jerry"));
        assert!(cards[0].1.contains("+61411112222"));
        assert!(!cards[0].1.contains("&#"));
        let escaped = "<href>Tom&#13\\;</href>";
        assert_eq!(xml_hrefs(escaped)[0], "Tom");
    }

    #[test]
    fn href_with_attributes() {
        let xml = r#"<d:href xmlns:d="DAV:">/abc/card/x.vcf</d:href>"#;
        let hs = xml_hrefs(xml);
        assert_eq!(hs[0], "/abc/card/x.vcf");
    }

    #[test]
    fn principal_nested_href() {
        let xml = r#"<d:multistatus xmlns:d="DAV:">
          <d:response>
            <d:href>/</d:href>
            <d:propstat>
              <d:prop>
                <d:current-user-principal>
                  <d:href>/12345/principal/</d:href>
                </d:current-user-principal>
              </d:prop>
            </d:propstat>
          </d:response>
        </d:multistatus>"#;
        assert_eq!(href_in_prop(xml, "current-user-principal").as_deref(), Some("/12345/principal/"));
    }

    #[test]
    fn picks_addressbook_collection() {
        let xml = r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
          <d:response>
            <d:href>/12345/carddavhome/</d:href>
            <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat>
          </d:response>
          <d:response>
            <d:href>/12345/carddavhome/card/</d:href>
            <d:propstat><d:prop><d:resourcetype><d:collection/><c:addressbook/></d:resourcetype></d:prop></d:propstat>
          </d:response>
        </d:multistatus>"#;
        let books = addressbook_hrefs(xml);
        assert_eq!(books, vec!["/12345/carddavhome/card/".to_string()]);
    }

    #[test]
    fn partition_host_from_icloud_headers() {
        assert_eq!(
            icloud_contacts_host("60", "").as_deref(),
            Some("p60-contacts.icloud.com")
        );
        assert_eq!(
            icloud_contacts_host("", "carddav:3:p43-carddav-868b76885f-ntn7b:8080").as_deref(),
            Some("p43-contacts.icloud.com")
        );
    }

    #[test]
    fn principal_from_icloud_style_xml() {
        let xml = r#"<multistatus xmlns="DAV:">
          <response>
            <href>/.well-known/carddav</href>
            <propstat>
              <prop>
                <current-user-principal>
                  <href>/112233/principal/</href>
                </current-user-principal>
              </prop>
            </propstat>
          </response>
        </multistatus>"#;
        assert_eq!(pick_principal(xml).as_deref(), Some("/112233/principal/"));
    }
}
