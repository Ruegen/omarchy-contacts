use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::contact::{Address, Contact, Email, Link, Phone};
use crate::csv_io;
use crate::fsutil::{is_safe_name, open_nofollow_abs, atomic_write_abs};
use crate::paths::{uid_file_name, Layout, MAX_IMPORT_BYTES, MAX_IPC_LINE, MAX_LOG_BYTES, MIN_SYNC_SECS};
use crate::secrets;
use crate::store::Store;
use crate::sync::icloud::{interval, IcloudSync, SyncProgress};
use crate::vcard::serialize_card;
use crate::watch::DropWatch;

pub struct Daemon {
    store: Store,
    config: Config,
    watch: DropWatch,
    last_sync: Option<Instant>,
    last_attempt: Option<Instant>,
    syncing: bool,
    progress: SyncProgress,
}

impl Daemon {
    pub fn new() -> io::Result<Self> {
        let store = Store::open()?;
        let config = load_config(&store.layout)?;
        Ok(Self {
            store,
            config,
            watch: DropWatch::new(),
            last_sync: None,
            last_attempt: None,
            syncing: false,
            progress: SyncProgress::default(),
        })
    }

    pub fn handle_line(&mut self, line: &str) -> Value {
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => return json!({"ok": false, "error": "bad json"}),
        };
        let id = v.get("id").cloned().unwrap_or(Value::Null);
        let cmd = v.get("cmd").and_then(|c| c.as_str()).unwrap_or("");
        let mut out = self.dispatch(cmd, &v);
        out["id"] = id;
        out
    }

    fn dispatch(&mut self, cmd: &str, v: &Value) -> Value {
        match cmd {
            "ping" => json!({"ok": true, "pong": true}),
            "status" => self.status(),
            "list" => self.list(v.get("query").and_then(|q| q.as_str()).unwrap_or("")),
            "get" => self.get(v.get("uid").and_then(|q| q.as_str()).unwrap_or("")),
            "save" => self.save(v.get("contact")),
            "delete" => self.delete(v.get("uid").and_then(|q| q.as_str()).unwrap_or("")),
            "import_vcf" => self.import_path(v.get("path").and_then(|q| q.as_str()).unwrap_or(""), false),
            "export_vcf" => self.export_path(v, false),
            "import_csv" => self.import_path(v.get("path").and_then(|q| q.as_str()).unwrap_or(""), true),
            "export_csv" => self.export_path(v, true),
            "email_card" => self.email_card(v.get("uid").and_then(|q| q.as_str()).unwrap_or("")),
            "sync_now" => self.sync_now(),
            "set_icloud" => self.set_icloud(v),
            "clear_icloud" => self.clear_icloud(),
            "get_config" => json!({"ok": true, "config": self.config}),
            "set_config" => self.set_config(v),
            "tick" => self.tick(),
            _ => json!({"ok": false, "error": "unknown command"}),
        }
    }

    fn status(&self) -> Value {
        json!({
            "ok": true,
            "count": self.store.count(),
            "sync": {
                "enabled": self.config.sync.enabled,
                "interval_secs": self.config.sync.interval_secs,
                "provider": self.config.sync.provider,
                "apple_id": self.config.sync.apple_id,
                "has_password": secrets::load_password().is_ok(),
                "syncing": self.syncing,
                "progress": self.progress,
            },
            "keys": self.config.keys,
            "watch": self.config.watch.enabled,
        })
    }

    fn list(&self, q: &str) -> Value {
        let q = q.chars().take(200).collect::<String>();
        let contacts: Vec<Value> = self.store.search(&q).into_iter().map(|c| c.for_list()).collect();
        json!({"ok": true, "contacts": contacts, "count": self.store.count()})
    }

    fn get(&self, uid: &str) -> Value {
        match uid_file_name(uid) {
            Ok(_) => {}
            Err(_) => return json!({"ok": false, "error": "bad id"}),
        }
        match self.store.get(uid) {
            Some(c) => json!({"ok": true, "contact": c.for_detail()}),
            None => json!({"ok": false, "error": "not found"}),
        }
    }

    fn save(&mut self, contact: Option<&Value>) -> Value {
        let Some(v) = contact else {
            return json!({"ok": false, "error": "missing contact"});
        };
        match contact_from_json(v) {
            Ok(c) => match self.store.upsert(c) {
                Ok(saved) => json!({"ok": true, "contact": saved.for_detail(), "count": self.store.count()}),
                Err(e) => json!({"ok": false, "error": e.to_string()}),
            },
            Err(e) => json!({"ok": false, "error": e}),
        }
    }

    fn delete(&mut self, uid: &str) -> Value {
        match self.store.delete(uid) {
            Ok(true) => json!({"ok": true, "count": self.store.count()}),
            Ok(false) => json!({"ok": false, "error": "not found"}),
            Err(e) => json!({"ok": false, "error": e.to_string()}),
        }
    }

    fn import_path(&mut self, path: &str, csv: bool) -> Value {
        match read_user_file(path, MAX_IMPORT_BYTES) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                if csv {
                    match csv_io::import_csv(&mut self.store, &text) {
                        Ok((ok, skipped)) => json!({"ok": true, "imported": ok, "skipped": skipped, "count": self.store.count()}),
                        Err(e) => json!({"ok": false, "error": e.to_string()}),
                    }
                } else {
                    match self.store.import_text(&text) {
                        Ok(r) => json!({"ok": true, "imported": r.ok, "skipped": r.skipped, "count": self.store.count()}),
                        Err(e) => json!({"ok": false, "error": e.to_string()}),
                    }
                }
            }
            Err(e) => json!({"ok": false, "error": e.to_string()}),
        }
    }

    fn export_path(&self, v: &Value, csv: bool) -> Value {
        let path = v.get("path").and_then(|q| q.as_str()).unwrap_or("");
        let uids: Option<Vec<String>> = v.get("uids").and_then(|u| {
            u.as_array().map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
        });
        let data = if csv {
            match csv_io::export_csv(&self.store, uids.as_deref()) {
                Ok(s) => s.into_bytes(),
                Err(e) => return json!({"ok": false, "error": e.to_string()}),
            }
        } else {
            self.store.export_text(uids.as_deref()).into_bytes()
        };
        match write_user_file(path, &data) {
            Ok(()) => json!({"ok": true, "bytes": data.len()}),
            Err(e) => json!({"ok": false, "error": e.to_string()}),
        }
    }

    fn email_card(&self, uid: &str) -> Value {
        let Some(c) = self.store.get(uid) else {
            return json!({"ok": false, "error": "not found"});
        };
        let name = export_file_name(c);
        let text = serialize_card(c);
        if let Err(e) = self.store.layout.runtime.atomic_write(&name, text.as_bytes()) {
            return json!({"ok": false, "error": e.to_string()});
        }
        let path = self.store.layout.runtime_path.join(&name);
        match mail_card_file(&path, &c.display_name()) {
            Ok(()) => json!({"ok": true, "path": path.to_string_lossy()}),
            Err(e) => json!({"ok": false, "error": e}),
        }
    }

    fn set_icloud(&mut self, v: &Value) -> Value {
        let apple_id = v.get("apple_id").and_then(|s| s.as_str()).unwrap_or("").trim();
        let password = v.get("password").and_then(|s| s.as_str()).unwrap_or("");
        if apple_id.is_empty() || !apple_id.contains('@') {
            return json!({"ok": false, "error": "Apple ID looks wrong"});
        }
        if password.is_empty() {
            if secrets::load_password().is_err() {
                return json!({"ok": false, "error": "App-specific password is missing"});
            }
        } else {
            if let Err(e) = secrets::reject_regular_password(password) {
                return json!({"ok": false, "error": e});
            }
            if let Err(e) = secrets::store_password(password) {
                return json!({"ok": false, "error": e.to_string()});
            }
        }
        self.config.sync.apple_id = apple_id.to_string();
        self.config.sync.enabled = true;
        self.config.sync.provider = "icloud".into();
        if let Err(e) = save_config(&self.store.layout, &self.config) {
            return json!({"ok": false, "error": e.to_string()});
        }
        json!({"ok": true, "apple_id": self.config.sync.apple_id})
    }

    fn clear_icloud(&mut self) -> Value {
        let _ = secrets::clear_password();
        self.config.sync.enabled = false;
        self.config.sync.apple_id.clear();
        let _ = save_config(&self.store.layout, &self.config);
        json!({"ok": true})
    }

    fn set_config(&mut self, v: &Value) -> Value {
        if let Some(n) = v.get("interval_secs").and_then(|x| x.as_u64()) {
            self.config.sync.interval_secs = n.max(MIN_SYNC_SECS);
        }
        if let Some(b) = v.get("watch").and_then(|x| x.as_bool()) {
            self.config.watch.enabled = b;
        }
        if let Some(keys) = v.get("keys") {
            if let Ok(k) = serde_json::from_value(keys.clone()) {
                self.config.keys = k;
            }
        }
        self.config.clamp();
        let _ = save_config(&self.store.layout, &self.config);
        json!({"ok": true, "config": self.config})
    }

    fn sync_now(&mut self) -> Value {
        if self.syncing {
            return json!({"ok": true, "progress": self.progress});
        }
        if !self.config.sync.enabled {
            return json!({"ok": false, "error": "iCloud is not configured"});
        }
        match self.run_sync() {
            Ok(p) => json!({"ok": true, "progress": p, "count": self.store.count()}),
            Err(e) => json!({"ok": false, "error": e.to_string(), "progress": self.progress}),
        }
    }

    fn run_sync(&mut self) -> io::Result<SyncProgress> {
        if let Some(last) = self.last_sync {
            if last.elapsed() < Duration::from_secs(MIN_SYNC_SECS) {
                return Err(io::Error::other("sync is cooling down (2 min minimum)"));
            }
        }
        self.last_attempt = Some(Instant::now());
        let password = secrets::load_password()?;
        let apple_id = self.config.sync.apple_id.clone();
        self.syncing = true;
        self.progress = SyncProgress {
            phase: "discover".into(),
            done: 0,
            total: 0,
            error: String::new(),
        };
        let mut client = IcloudSync::connect(&apple_id, &password)?;
        emit_progress(&self.progress);
        let mut log_buf = Vec::new();
        let mut latest = self.progress.clone();
        let result = client.run(&mut self.store, &mut log_buf, |p, store| {
            latest = p.clone();
            emit_progress(&p);
            emit_list(store);
        });
        self.progress = latest;
        append_log(&self.store.layout, &log_buf);
        self.syncing = false;
        match result {
            Ok(p) => {
                self.last_sync = Some(Instant::now());
                self.progress = p.clone();
                emit_progress(&self.progress);
                Ok(p)
            }
            Err(e) => {
                self.progress.phase = "idle".into();
                self.progress.error = e.to_string();
                emit_progress(&self.progress);
                Err(e)
            }
        }
    }

    fn tick(&mut self) -> Value {
        let mut imported = 0u32;
        if self.config.watch.enabled {
            if let Some((n, _)) = self.watch.poll(&mut self.store) {
                imported = n;
            }
        }
        let mut synced = false;
        if self.config.sync.enabled && !self.syncing {
            let cooling = self
                .last_attempt
                .map(|t| t.elapsed() < Duration::from_secs(MIN_SYNC_SECS))
                .unwrap_or(false);
            let due = match self.last_sync {
                None => !cooling,
                Some(t) => t.elapsed() >= interval(&self.config) && !cooling,
            };
            if due {
                synced = self.run_sync().is_ok();
            }
        }
        json!({
            "ok": true,
            "imported": imported,
            "synced": synced,
            "count": self.store.count(),
            "progress": self.progress,
        })
    }
}

pub fn serve_stdio(daemon: &mut Daemon) -> io::Result<()> {
    serve_stdio_shared(Arc::new(Mutex::new(std::mem::replace(
        daemon,
        Daemon::new()?,
    ))))
}

pub fn serve_stdio_shared(daemon: Arc<Mutex<Daemon>>) -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        if line.len() > MAX_IPC_LINE {
            return Err(io::Error::other("line too long"));
        }
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed.is_empty() {
            continue;
        }
        let out = {
            let mut d = daemon.lock().map_err(|_| io::Error::other("lock"))?;
            d.handle_line(trimmed)
        };
        let mut s = serde_json::to_string(&out).unwrap_or_else(|_| "{\"ok\":false}".into());
        if s.len() > MAX_IPC_LINE {
            s = json!({"ok": false, "error": "response too large", "id": out.get("id")}).to_string();
        }
        stdout.write_all(s.as_bytes())?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}

pub fn serve_socket(path: &Path, daemon: &mut Daemon) -> io::Result<()> {
    serve_socket_shared(path, Arc::new(Mutex::new(std::mem::replace(daemon, Daemon::new()?))))
}

pub fn serve_socket_shared(path: &Path, daemon: Arc<Mutex<Daemon>>) -> io::Result<()> {
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path)?;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    listener.set_nonblocking(false)?;
    for stream in listener.incoming() {
        let mut stream = stream?;
        if !peer_is_us(&stream) {
            continue;
        }
        let daemon = Arc::clone(&daemon);
        if handle_stream_shared(&mut stream, daemon).is_err() {
            continue;
        }
    }
    Ok(())
}

fn handle_stream_shared(stream: &mut UnixStream, daemon: Arc<Mutex<Daemon>>) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        if line.len() > MAX_IPC_LINE {
            break;
        }
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed.is_empty() {
            continue;
        }
        let out = {
            let mut d = daemon.lock().map_err(|_| io::Error::other("lock"))?;
            d.handle_line(trimmed)
        };
        let s = serde_json::to_string(&out).unwrap_or_else(|_| "{\"ok\":false}".into());
        stream.write_all(s.as_bytes())?;
        stream.write_all(b"\n")?;
        stream.flush()?;
    }
    Ok(())
}

fn peer_is_us(stream: &UnixStream) -> bool {
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut _ as *mut _,
            &mut len,
        )
    };
    rc == 0 && cred.uid == unsafe { libc::geteuid()     }
}

fn emit_progress(p: &SyncProgress) {
    let line = json!({"ok": true, "progress": p});
    let mut out = io::stdout();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn emit_list(store: &Store) {
    let contacts: Vec<Value> = store
        .search("")
        .into_iter()
        .map(|c| c.for_list())
        .collect();
    let line = json!({"ok": true, "contacts": contacts, "count": store.count()});
    let mut out = io::stdout();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn load_config(layout: &Layout) -> io::Result<Config> {
    match layout.config.read_capped("config.toml", 64 * 1024) {
        Ok(bytes) => Ok(Config::from_toml(&String::from_utf8_lossy(&bytes))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(e),
    }
}

fn save_config(layout: &Layout, cfg: &Config) -> io::Result<()> {
    layout.config.atomic_write("config.toml", cfg.to_toml().as_bytes())
}

fn append_log(layout: &Layout, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let existing = layout
        .data
        .read_capped("sync.log", MAX_LOG_BYTES)
        .unwrap_or_default();
    let mut out = existing;
    out.extend_from_slice(bytes);
    if out.len() > MAX_LOG_BYTES {
        out = out[out.len() - MAX_LOG_BYTES / 2..].to_vec();
    }
    let _ = layout.data.atomic_write("sync.log", &out);
}

fn read_user_file(path: &str, max: usize) -> io::Result<Vec<u8>> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(io::Error::other("path must be absolute"));
    }
    let name = p
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| io::Error::other("bad name"))?;
    if !is_safe_name(name) && !name.starts_with('.') {
        // Allow typical export names; still reject separators via Path.
        if name.contains("..") {
            return Err(io::Error::other("bad name"));
        }
    }
    let lower = name.to_ascii_lowercase();
    if !(lower.ends_with(".vcf") || lower.ends_with(".csv") || lower.ends_with(".vcard")) {
        return Err(io::Error::other("expected a .vcf or .csv file"));
    }
    open_nofollow_abs(p, max)
}

fn write_user_file(path: &str, bytes: &[u8]) -> io::Result<()> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(io::Error::other("path must be absolute"));
    }
    let name = p
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| io::Error::other("bad name"))?;
    let lower = name.to_ascii_lowercase();
    if !(lower.ends_with(".vcf") || lower.ends_with(".csv") || lower.ends_with(".vcard")) {
        return Err(io::Error::other("expected a .vcf or .csv file"));
    }
    atomic_write_abs(p, bytes)
}

fn contact_from_json(v: &Value) -> Result<Contact, String> {
    let mut c = Contact {
        uid: v.get("uid").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        first: v.get("first").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        last: v.get("last").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        fn_: v.get("fn").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        nickname: v.get("nickname").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        org: v.get("org").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        title: v.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        note: v.get("note").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        bday: v.get("bday").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        ..Contact::default()
    };
    if let Some(arr) = v.get("phones").and_then(|x| x.as_array()) {
        for p in arr {
            c.phones.push(Phone {
                type_: p.get("type").and_then(|x| x.as_str()).unwrap_or("other").to_string(),
                value: p.get("value").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            });
        }
    }
    if let Some(arr) = v.get("emails").and_then(|x| x.as_array()) {
        for p in arr {
            c.emails.push(Email {
                type_: p.get("type").and_then(|x| x.as_str()).unwrap_or("other").to_string(),
                value: p.get("value").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            });
        }
    }
    if let Some(arr) = v.get("urls").and_then(|x| x.as_array()) {
        for p in arr {
            c.urls.push(Link {
                type_: p.get("type").and_then(|x| x.as_str()).unwrap_or("other").to_string(),
                value: p.get("value").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            });
        }
    }
    if let Some(arr) = v.get("addresses").and_then(|x| x.as_array()) {
        for p in arr {
            c.addresses.push(Address {
                type_: p.get("type").and_then(|x| x.as_str()).unwrap_or("other").to_string(),
                street: p.get("street").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                city: p.get("city").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                region: p.get("region").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                postal: p.get("postal").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                country: p.get("country").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            });
        }
    }
    if let Some(arr) = v.get("groups").and_then(|x| x.as_array()) {
        for g in arr {
            if let Some(s) = g.as_str() {
                let s = s.trim();
                if !s.is_empty() {
                    c.groups.push(s.to_string());
                }
            }
        }
    }
    if let Some(arr) = v.get("members").and_then(|x| x.as_array()) {
        for m in arr {
            if let Some(s) = m.as_str() {
                let s = s.trim();
                if !s.is_empty() {
                    c.members.push(s.to_string());
                }
            }
        }
    }
    if v.get("is_group").and_then(|x| x.as_bool()).unwrap_or(false) {
        c.is_group = true;
    }
    if let Some(b64) = v.get("photo_b64").and_then(|x| x.as_str()) {
        if !b64.is_empty() {
            let compact: String = b64.chars().filter(|ch| !ch.is_whitespace()).collect();
            if compact.len() > crate::paths::MAX_PHOTO_BYTES * 2 {
                return Err("photo too large".into());
            }
            if let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &compact) {
                if bytes.len() <= crate::paths::MAX_PHOTO_BYTES {
                    c.photo_jpeg = Some(bytes);
                }
            }
        }
    }
    if c.note.len() > 16 * 1024 {
        c.note.truncate(16 * 1024);
    }
    c.normalize();
    Ok(c)
}

fn export_file_name(c: &Contact) -> String {
    let raw = c.display_name();
    let mut s = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            s.push(ch);
        } else if ch == ' ' || ch == '-' {
            if !s.ends_with('-') {
                s.push('-');
            }
        }
        if s.len() >= 60 {
            break;
        }
    }
    let s = s.trim_matches('-').to_string();
    let file = format!("{}.vcf", if s.is_empty() { "contact".into() } else { s });
    if is_safe_name(&file) {
        file
    } else {
        "contact.vcf".into()
    }
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn mail_card_file(path: &Path, name: &str) -> Result<(), String> {
    if !path.is_file() {
        return Err("The card file could not be written.".into());
    }
    let path_json = json_string(&path.to_string_lossy());
    let subject = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-')
        .take(40)
        .collect::<String>();
    let payload = format!(
        r#"{{"compose":true,"attachments":[{path_json}],"mailto":"mailto:?subject=Contact {subject}"}}"#
    );
    let summoned = Command::new("omarchy-shell")
        .args(["shell", "summon", "omamail", &payload])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if summoned {
        return Ok(());
    }
    Command::new("xdg-email")
        .arg("--attach")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("Could not open mail with the card attached: {err}"))
}

