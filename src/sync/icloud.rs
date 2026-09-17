use std::io::{self, Write};
use std::time::Duration;

use crate::config::Config;
use crate::contact::Contact;
use crate::store::Store;
use crate::sync::carddav::{
    addressbook_hrefs, card_hrefs, contacts_host_from_xml, href_in_prop, icloud_contacts_host, join_url,
    pick_principal, vcards_in_multistatus, xml_hrefs, xml_shape, CardDavClient, DavResponse,
    ADDRESSBOOK_HOME, ADDRESSBOOK_INDEX, ADDRESSBOOK_LIST, ADDRESSBOOK_QUERY, CURRENT_USER_PRINCIPAL,
};
use crate::vcard::{parse_card, rev_newer};

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

    pub fn discover(&mut self, log: &mut impl Write) -> io::Result<String> {
        let _ = writeln!(log, "{} discover start", stamp());
        let mut principal_res = self.client.propfind_at(WELL_KNOWN, "0", CURRENT_USER_PRINCIPAL)?;
        self.log_hop(log, "discover", &principal_res)?;
        let mut principal = principal_from(&principal_res);

        if principal.is_none() {
            let xml = String::from_utf8_lossy(&principal_res.body);
            let hop = icloud_contacts_host(&principal_res.partition, &principal_res.instance)
                .or_else(|| contacts_host_from_xml(&xml));
            if let Some(host) = hop {
                let next = format!("https://{host}/");
                if host_only(&principal_res.url) != host {
                    let _ = writeln!(log, "{} discover hop host={}", stamp(), host);
                    principal_res = self.client.propfind_at(&next, "0", CURRENT_USER_PRINCIPAL)?;
                    self.log_hop(log, "discover", &principal_res)?;
                    principal = principal_from(&principal_res);
                }
            }
        }

        if principal.is_none() {
            if host_only(&principal_res.url) != "contacts.icloud.com"
                || principal_res.url.contains("well-known")
            {
                let _ = writeln!(log, "{} discover retry host=contacts.icloud.com", stamp());
                principal_res = self.client.propfind_at("https://contacts.icloud.com/", "0", CURRENT_USER_PRINCIPAL)?;
                self.log_hop(log, "discover", &principal_res)?;
                principal = principal_from(&principal_res);
            }
        }

        if principal_res.status == 401 || principal_res.status == 403 {
            return Err(io::Error::other(
                "iCloud refused the sign-in. Check the Apple ID and app-specific password.",
            ));
        }
        let principal = principal.ok_or_else(|| {
            io::Error::other("iCloud did not return an account path")
        })?;
        let principal_url = join_url(&principal_res.url, &principal);
        let _ = writeln!(log, "{} principal host={}", stamp(), host_only(&principal_url));

        let home_res = self.client.propfind_at(&principal_url, "0", ADDRESSBOOK_HOME)?;
        if home_res.status == 401 || home_res.status == 403 {
            return Err(io::Error::other(
                "iCloud refused the sign-in. Check the Apple ID and app-specific password.",
            ));
        }
        if !dav_ok(&home_res) {
            return Err(io::Error::other(format!(
                "could not find your address book home ({})",
                home_res.status
            )));
        }
        let home_xml = String::from_utf8_lossy(&home_res.body);
        let home = href_in_prop(&home_xml, "addressbook-home-set")
            .or_else(|| xml_hrefs(&home_xml).into_iter().find(|h| h != &principal))
            .ok_or_else(|| io::Error::other("no addressbook home"))?;
        let home_url = join_url(&home_res.url, &home);

        let list_res = self.client.propfind_at(&home_url, "1", ADDRESSBOOK_LIST)?;
        if list_res.status == 401 || list_res.status == 403 {
            return Err(io::Error::other(
                "iCloud refused the sign-in. Check the Apple ID and app-specific password.",
            ));
        }
        if !dav_ok(&list_res) {
            return Err(io::Error::other(format!(
                "could not list address books ({})",
                list_res.status
            )));
        }
        let list_xml = String::from_utf8_lossy(&list_res.body);
        let book = addressbook_hrefs(&list_xml)
            .into_iter()
            .next()
            .or_else(|| {
                xml_hrefs(&list_xml)
                    .into_iter()
                    .find(|h| join_url(&home_url, h) != home_url && join_url(&home_url, h).trim_end_matches('/') != home_url.trim_end_matches('/'))
            })
            .ok_or_else(|| io::Error::other("no address book"))?;
        let book_url = join_url(&list_res.url, &book);
        let _ = writeln!(log, "{} book host={}", stamp(), host_only(&book_url));
        self.book = Some(book_url.clone());
        Ok(book_url)
    }

    fn log_hop(&self, log: &mut impl Write, phase: &str, res: &DavResponse) -> io::Result<()> {
        let xml = String::from_utf8_lossy(&res.body);
        let _ = writeln!(
            log,
            "{} {} http {} host={} {}",
            stamp(),
            phase,
            res.status,
            host_only(&res.url),
            xml_shape(&xml)
        );
        Ok(())
    }

    pub fn book_url(&mut self, log: &mut impl Write) -> io::Result<String> {
        if let Some(b) = &self.book {
            return Ok(b.clone());
        }
        self.discover(log)
    }

    fn ingest(
        &self,
        store: &mut Store,
        log: &mut impl Write,
        url: String,
        etag: String,
        contact: Contact,
        remote_uids: &mut std::collections::HashSet<String>,
    ) -> io::Result<()> {
        let mut contact = contact;
        contact.href = Some(url.clone());
        if !etag.is_empty() {
            contact.etag = Some(etag);
        }
        contact.normalize();
        remote_uids.insert(contact.uid.clone());
        if let Some(local) = store.get(&contact.uid).cloned() {
            if rev_newer(&local.rev, &contact.rev) {
                let put_url = local.href.clone().unwrap_or(url);
                match self.client.put_card(&put_url, &local, local.etag.as_deref()) {
                    Ok(_) => {
                        let _ = writeln!(log, "{} push uid={}", stamp(), log_uid(&local.uid));
                    }
                    Err(e) => {
                        let _ = writeln!(log, "{} push fail {}", stamp(), e);
                    }
                }
                return Ok(());
            }
        }
        store.replace_from_remote(contact)
    }

    pub fn run(
        &mut self,
        store: &mut Store,
        log: &mut impl Write,
        mut progress: impl FnMut(SyncProgress, &Store),
    ) -> io::Result<SyncProgress> {
        let book = self.book_url(log)?;
        let _ = writeln!(log, "{} pull start host={}", stamp(), host_only(&book));
        progress(SyncProgress {
            phase: "listing".into(),
            done: 0,
            total: 0,
            error: String::new(),
        }, store);

        let mut remote_uids = std::collections::HashSet::new();
        let mut pulled = 0u32;

        let query = self.client.report_at(&book, ADDRESSBOOK_QUERY);
        let bulk = match query {
            Ok(res) if res.status == 207 || res.status == 200 => {
                let xml = String::from_utf8_lossy(&res.body);
                vcards_in_multistatus(&xml)
            }
            Ok(res) => {
                let _ = writeln!(log, "{} report status={}", stamp(), res.status);
                Vec::new()
            }
            Err(e) => {
                let _ = writeln!(log, "{} report skip {}", stamp(), e);
                Vec::new()
            }
        };

        if !bulk.is_empty() {
            let hrefs: Vec<String> = bulk.iter().map(|(h, _)| h.clone()).collect();
            let total = bulk.len() as u32;
            progress(SyncProgress {
                phase: "pull".into(),
                done: 0,
                total,
                error: String::new(),
            }, store);
            for (i, (href, text)) in bulk.into_iter().enumerate() {
                let url = join_url(&book, &href);
                match parse_card(&text) {
                    Ok(contact) => {
                        if let Err(e) = self.ingest(store, log, url, String::new(), contact, &mut remote_uids)
                        {
                            let _ = writeln!(log, "{} save fail {}", stamp(), e);
                        } else {
                            pulled += 1;
                        }
                    }
                    Err(e) => {
                        let _ = writeln!(log, "{} parse fail {}", stamp(), e);
                    }
                }
                progress(SyncProgress {
                    phase: "pull".into(),
                    done: (i + 1) as u32,
                    total,
                    error: String::new(),
                }, store);
            }
            // The fast listing omits photos. Fetch each card so pictures land.
            let total = hrefs.len() as u32;
            progress(SyncProgress {
                phase: "photos".into(),
                done: 0,
                total,
                error: String::new(),
            }, store);
            for (i, href) in hrefs.iter().enumerate() {
                let url = join_url(&book, href);
                match self.client.get_card(&url) {
                    Ok(remote) => {
                        if let Err(e) = self.ingest(
                            store,
                            log,
                            url,
                            remote.etag,
                            remote.contact,
                            &mut remote_uids,
                        ) {
                            let _ = writeln!(log, "{} photo fail {}", stamp(), e);
                        }
                    }
                    Err(e) => {
                        let _ = writeln!(log, "{} get fail {}", stamp(), e);
                    }
                }
                progress(SyncProgress {
                    phase: "photos".into(),
                    done: (i + 1) as u32,
                    total,
                    error: String::new(),
                }, store);
            }
        } else {
            let index = self.client.propfind_at(&book, "1", ADDRESSBOOK_INDEX)?;
            if index.status == 401 || index.status == 403 {
                return Err(io::Error::other(
                    "iCloud refused the sign-in. Check the Apple ID and app-specific password.",
                ));
            }
            if index.status != 207 && index.status != 200 {
                return Err(io::Error::other(format!("could not list contacts ({})", index.status)));
            }
            let xml = String::from_utf8_lossy(&index.body);
            let hrefs = card_hrefs(&xml, &book);
            let total = hrefs.len() as u32;
            let _ = writeln!(log, "{} index n={total}", stamp());
            progress(SyncProgress {
                phase: "pull".into(),
                done: 0,
                total,
                error: String::new(),
            }, store);
            for (i, href) in hrefs.iter().enumerate() {
                let url = join_url(&book, href);
                match self.client.get_card(&url) {
                    Ok(remote) => {
                        if let Err(e) = self.ingest(
                            store,
                            log,
                            url,
                            remote.etag,
                            remote.contact,
                            &mut remote_uids,
                        ) {
                            let _ = writeln!(log, "{} save fail {}", stamp(), e);
                        } else {
                            pulled += 1;
                        }
                    }
                    Err(e) => {
                        let _ = writeln!(log, "{} get fail {}", stamp(), e);
                    }
                }
                progress(SyncProgress {
                    phase: "pull".into(),
                    done: (i + 1) as u32,
                    total,
                    error: String::new(),
                }, store);
            }
        }

        let locals: Vec<Contact> = store
            .all()
            .into_iter()
            .filter(|c| !remote_uids.contains(&c.uid))
            .cloned()
            .collect();
        for c in locals {
            let url = format!("{}/{}.vcf", book.trim_end_matches('/'), c.uid);
            match self.client.put_card(&url, &c, None) {
                Ok(_) => {
                    let _ = writeln!(log, "{} push new uid={}", stamp(), log_uid(&c.uid));
                }
                Err(e) => {
                    let _ = writeln!(log, "{} push new fail {}", stamp(), e);
                }
            }
        }
        let _ = writeln!(log, "{} pull done n={pulled}", stamp());
        if pulled == 0 && !remote_uids.is_empty() {
            return Err(io::Error::other(
                "iCloud sent contacts, but none could be saved",
            ));
        }
        Ok(SyncProgress {
            phase: "idle".into(),
            done: pulled,
            total: pulled,
            error: String::new(),
        })
    }
}

fn dav_ok(res: &DavResponse) -> bool {
    res.status == 207 || res.status == 200
}

fn principal_from(res: &DavResponse) -> Option<String> {
    if !dav_ok(res) {
        return None;
    }
    pick_principal(&String::from_utf8_lossy(&res.body))
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
