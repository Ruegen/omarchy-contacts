use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::contact::{Contact, Email, Phone};
use crate::csv_io;
use crate::fsutil::{is_safe_name, open_nofollow_abs, atomic_write_abs};
use crate::paths::{uid_file_name, Layout, MAX_IMPORT_BYTES, MAX_IPC_LINE, MAX_LOG_BYTES, MIN_SYNC_SECS};
use crate::secrets;
use crate::store::Store;
use crate::sync::icloud::{interval, IcloudSync, SyncProgress};
use crate::watch::DropWatch;

pub struct Daemon {
    store: Store,
    config: Config,
    watch: DropWatch,
    last_sync: Option<Instant>,
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

    fn set_icloud(&mut self, v: &Value) -> Value {
        let apple_id = v.get("apple_id").and_then(|s| s.as_str()).unwrap_or("").trim();
        let password = v.get("password").and_then(|s| s.as_str()).unwrap_or("");
        if apple_id.is_empty() || !apple_id.contains('@') {
            return json!({"ok": false, "error": "Apple ID looks wrong"});
        }
        if let Err(e) = secrets::reject_regular_password(password) {
            return json!({"ok": false, "error": e});
        }
        if let Err(e) = secrets::store_password(password) {
            return json!({"ok": false, "error": e.to_string()});
        }
        self.config.sync.apple_id = apple_id.to_string();
        self.config.sync.enabled = true;
        self.config.sync.provider = "icloud".into();
        let _ = save_config(&self.store.layout, &self.config);
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
        let mut log_buf = Vec::new();
        let result = client.run(&mut self.store, &mut log_buf, |p| {
            self.progress = p;
        });
        append_log(&self.store.layout, &log_buf);
        self.syncing = false;
        self.last_sync = Some(Instant::now());
        match result {
            Ok(p) => {
                self.progress = p.clone();
                Ok(p)
            }
            Err(e) => {
                self.progress.error = "sync failed".into();
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
            let due = match self.last_sync {
                None => true,
                Some(t) => t.elapsed() >= interval(&self.config),
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
    rc == 0 && cred.uid == unsafe { libc::geteuid() }
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

