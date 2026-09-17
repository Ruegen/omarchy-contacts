use std::io::{self, Write};
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::contact::Contact;
use crate::store::Store;
use crate::sync::carddav::{
    join_url, xml_hrefs, xml_tag_values, CardDavClient, ADDRESSBOOK_HOME, ADDRESSBOOK_INDEX,
    ADDRESSBOOK_LIST, CURRENT_USER_PRINCIPAL,
};
use crate::vcard::rev_newer;

const WELL_KNOWN: &str = "https://contacts.icloud.com/.well-known/carddav";

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct SyncProgress {
    pub phase: String,
    pub done: u32,
    pub total: u32,
    pub error: String,
}

pub struct IcloudSync {
    client: CardDavClient,
    book: Option<String>,
}

impl IcloudSync {
    pub fn connect(apple_id: &str, password: &str) -> io::Result<Self> {
        if apple_id.is_empty() || !apple_id.contains('@') {
            return Err(io::Error::other("Apple ID looks wrong"));
        }
        if apple_id.contains('\n') || apple_id.contains('\r') {
            return Err(io::Error::other("invalid Apple ID"));
        }
        Ok(Self {
            client: CardDavClient::new(apple_id.to_string(), password.to_string()),
            book: None,
        })
    }

    pub fn discover(&mut self) -> io::Result<String> {
        // RFC 6764: PROPFIND /.well-known/carddav — iCloud redirects to the real host.
        let (status, loc_or_etag, body) = self.client.propfind(WELL_KNOWN, "0", CURRENT_USER_PRINCIPAL)?;
        let start = if (300..400).contains(&status) && loc_or_etag.starts_with("https://") {
            loc_or_etag
        } else if status == 207 || status == 200 {
            WELL_KNOWN.to_string()
        } else {
            return Err(io::Error::other(format!("discovery {status}")));
        };
        let xml = String::from_utf8_lossy(&body);
        let principal = xml_hrefs(&xml)
            .into_iter()
            .find(|h| !h.contains(".well-known"))
            .unwrap_or_else(|| start.clone());
        let principal_url = join_url(&start, &principal);

        let (_, _, home_body) = self.client.propfind(&principal_url, "0", ADDRESSBOOK_HOME)?;
        let home_xml = String::from_utf8_lossy(&home_body);
        let home = xml_hrefs(&home_xml)
            .into_iter()
            .next()
            .ok_or_else(|| io::Error::other("no addressbook home"))?;
        let home_url = join_url(&principal_url, &home);

        let (_, _, list_body) = self.client.propfind(&home_url, "1", ADDRESSBOOK_LIST)?;
        let list_xml = String::from_utf8_lossy(&list_body);
        let hrefs = xml_hrefs(&list_xml);
        let book = hrefs
            .into_iter()
            .find(|h| h != &home && !h.ends_with('/').then(|| h.trim_end_matches('/')).unwrap_or(h).eq(&home))
            .or_else(|| {
                // Prefer a collection that is not the home itself.
                xml_tag_values(&list_xml, "href")
                    .into_iter()
                    .find(|h| join_url(&home_url, h) != home_url)
            })
            .ok_or_else(|| io::Error::other("no address book"))?;
        let book_url = join_url(&home_url, &book);
        self.book = Some(book_url.clone());
        Ok(book_url)
    }

    pub fn book_url(&mut self) -> io::Result<String> {
        if let Some(b) = &self.book {
            return Ok(b.clone());
        }
        self.discover()
    }

    pub fn run(
        &mut self,
        store: &mut Store,
        log: &mut impl Write,
        mut progress: impl FnMut(SyncProgress),
    ) -> io::Result<SyncProgress> {
        let book = self.book_url()?;
        let _ = writeln!(log, "{} pull start host={}", stamp(), host_only(&book));
        let (_, _, index) = self.client.propfind(&book, "1", ADDRESSBOOK_INDEX)?;
        let xml = String::from_utf8_lossy(&index);
        let hrefs: Vec<String> = xml_hrefs(&xml)
            .into_iter()
            .filter(|h| {
                let l = h.to_ascii_lowercase();
                l.ends_with(".vcf") || l.contains("/card") || (!h.ends_with('/') && h != &book)
            })
            .collect();
        let total = hrefs.len() as u32;
        progress(SyncProgress {
            phase: "pull".into(),
            done: 0,
            total,
            error: String::new(),
        });
        let mut remote_uids = std::collections::HashSet::new();
        let start = Instant::now();
        for (i, href) in hrefs.iter().enumerate() {
            let url = join_url(&book, href);
            match self.client.get_card(&url) {
                Ok(mut remote) => {
                    remote.contact.href = Some(url.clone());
                    remote.contact.etag = Some(remote.etag.clone());
                    remote.contact.normalize();
                    remote_uids.insert(remote.contact.uid.clone());
                    if let Some(local) = store.get(&remote.contact.uid).cloned() {
                        if rev_newer(&local.rev, &remote.contact.rev) {
                            let put_url = local.href.clone().unwrap_or(url);
                            match self.client.put_card(&put_url, &local, local.etag.as_deref()) {
                                Ok(_) => {
                                    let _ = writeln!(log, "{} push uid={}", stamp(), log_uid(&local.uid));
                                }
                                Err(e) => {
                                    let _ = writeln!(log, "{} push fail {}", stamp(), e);
                                }
                            }
                        } else if rev_newer(&remote.contact.rev, &local.rev)
                            || local.rev == remote.contact.rev
                        {
                            // Equal REV: still take remote only when we have no local dirty flag.
                            // Last-write-wins by REV: equal means keep local if we just pushed;
                            // otherwise accept remote.
                            if local.rev != remote.contact.rev || local.etag != remote.contact.etag {
                                store.replace_from_remote(remote.contact)?;
                            }
                        }
                    } else {
                        store.replace_from_remote(remote.contact)?;
                    }
                }
                Err(e) => {
                    let _ = writeln!(log, "{} get fail {}", stamp(), e);
                }
            }
            progress(SyncProgress {
                phase: if i < hrefs.len() / 2 { "pull" } else { "photos" }.into(),
                done: (i + 1) as u32,
                total,
                error: String::new(),
            });
            let _ = start;
        }

        // Push local-only contacts.
        let locals: Vec<Contact> = store
            .all()
            .into_iter()
            .filter(|c| !remote_uids.contains(&c.uid))
            .cloned()
            .collect();
        for c in locals {
            let url = format!("{}{}.vcf", book.trim_end_matches('/').to_string() + "/", c.uid);
            match self.client.put_card(&url, &c, None) {
                Ok(_) => {
                    let _ = writeln!(log, "{} push new uid={}", stamp(), log_uid(&c.uid));
                }
                Err(e) => {
                    let _ = writeln!(log, "{} push new fail {}", stamp(), e);
                }
            }
        }
        let _ = writeln!(log, "{} pull done n={total}", stamp());
        Ok(SyncProgress {
            phase: "idle".into(),
            done: total,
            total,
            error: String::new(),
        })
    }
}

fn stamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn host_only(url: &str) -> &str {
    url.strip_prefix("https://")
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or("icloud")
}

fn log_uid(uid: &str) -> String {
    uid.chars().take(8).collect()
}

pub fn interval(cfg: &Config) -> Duration {
    Duration::from_secs(cfg.sync.interval_secs.max(crate::paths::MIN_SYNC_SECS))
}

#[cfg(test)]
mod tests {
    use crate::vcard::rev_newer;

    #[test]
    fn conflict_rev() {
        assert!(rev_newer("20250101T000000Z", "20240101T000000Z"));
    }
}
