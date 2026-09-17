use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::fsutil::is_safe_name;
use crate::paths::MAX_IMPORT_BYTES;
use crate::store::Store;

pub struct DropWatch {
    seen: HashSet<String>,
    last_tick: u64,
}

impl DropWatch {
    pub fn new() -> Self {
        Self {
            seen: HashSet::new(),
            last_tick: 0,
        }
    }

    pub fn poll(&mut self, store: &mut Store) -> Option<(u32, String)> {
        let now = now_secs();
        if now.saturating_sub(self.last_tick) < 2 {
            return None;
        }
        self.last_tick = now;
        let names = store.layout.drop.list_names().ok()?;
        let mut imported = 0u32;
        let mut last = String::new();
        for name in names {
            if !name.to_ascii_lowercase().ends_with(".vcf") {
                continue;
            }
            if !is_safe_name(&name) {
                continue;
            }
            let key = name.clone();
            if self.seen.contains(&key) {
                continue;
            }
            match store.layout.drop.read_capped(&name, MAX_IMPORT_BYTES) {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    if let Ok(res) = store.import_text(&text) {
                        imported += res.ok;
                        last = name;
                        let _ = store.layout.drop.unlink(&key);
                    }
                    self.seen.insert(key);
                }
                Err(_) => {
                    self.seen.insert(key);
                }
            }
        }
        if imported > 0 {
            Some((imported, last))
        } else {
            None
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
