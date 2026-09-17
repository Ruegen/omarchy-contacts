use std::io;
use std::path::{Path, PathBuf};

use crate::fsutil::{is_safe_hidden_name, Dir};

pub const APP_DIR: &str = "omarchy-contacts";
pub const MAX_VCARD_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_IMPORT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_PHOTO_BYTES: usize = 1024 * 1024;
pub const MAX_IPC_LINE: usize = 2 * 1024 * 1024;
pub const MAX_LOG_BYTES: usize = 256 * 1024;
pub const MAX_CONTACTS: usize = 10_000;
pub const MIN_SYNC_SECS: u64 = 120;
pub const DEFAULT_SYNC_SECS: u64 = 300;

pub struct Layout {
    pub data: Dir,
    pub contacts: Dir,
    pub drop: Dir,
    pub config: Dir,
    pub runtime: Dir,
    pub data_path: PathBuf,
    pub config_path: PathBuf,
    pub runtime_path: PathBuf,
}

impl Layout {
    pub fn open() -> io::Result<Self> {
        let home = home_dir()?;
        let data_path = data_home()?.join(APP_DIR);
        let config_path = config_home()?.join(APP_DIR);
        let runtime_path = runtime_home()?.join(APP_DIR);

        let home_dir = Dir::open_path(&home)?;
        // Ensure parents exist by walking the resolved destinations.
        let data = mkdir_walk(&data_path)?;
        let contacts = data.open_or_mkdir("contacts", true)?;
        let drop = data.open_or_mkdir("import-drop", true)?;
        let config = mkdir_walk(&config_path)?;
        let runtime = mkdir_walk(&runtime_path)?;
        let _ = home_dir;

        Ok(Self {
            data,
            contacts,
            drop,
            config,
            runtime,
            data_path,
            config_path,
            runtime_path,
        })
    }

    pub fn socket_path(&self) -> PathBuf {
        self.runtime_path.join("contacts.sock")
    }

    pub fn log_name(&self) -> &'static str {
        "sync.log"
    }
}

pub fn home_dir() -> io::Result<PathBuf> {
    let uid = unsafe { libc::geteuid() };
    let pw = unsafe { libc::getpwuid(uid) };
    if pw.is_null() {
        return Err(io::Error::other("no passwd"));
    }
    let dir = unsafe { std::ffi::CStr::from_ptr((*pw).pw_dir) };
    let s = dir.to_str().map_err(|_| io::Error::other("home not utf8"))?;
    let p = PathBuf::from(s);
    if !p.is_absolute() {
        return Err(io::Error::other("home not absolute"));
    }
    Ok(p)
}

pub fn data_home() -> io::Result<PathBuf> {
    xdg("XDG_DATA_HOME", ".local/share")
}

pub fn config_home() -> io::Result<PathBuf> {
    xdg("XDG_CONFIG_HOME", ".config")
}

pub fn runtime_home() -> io::Result<PathBuf> {
    let raw = std::env::var("XDG_RUNTIME_DIR").map_err(|_| io::Error::other("XDG_RUNTIME_DIR unset"))?;
    if raw.is_empty() {
        return Err(io::Error::other("XDG_RUNTIME_DIR unset"));
    }
    let p = PathBuf::from(raw);
    if !p.is_absolute() {
        return Err(io::Error::other("XDG_RUNTIME_DIR not absolute"));
    }
    Ok(p)
}

fn xdg(var: &str, fallback: &str) -> io::Result<PathBuf> {
    if let Ok(v) = std::env::var(var) {
        if !v.is_empty() {
            let p = PathBuf::from(v);
            if p.is_absolute() {
                return Ok(p);
            }
        }
    }
    Ok(home_dir()?.join(fallback))
}

fn mkdir_walk(path: &Path) -> io::Result<Dir> {
    if !path.is_absolute() {
        return Err(io::Error::other("path must be absolute"));
    }
    let names: Vec<String> = path
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(n) => n.to_str().map(|s| s.to_string()),
            _ => None,
        })
        .collect();
    if names.iter().any(|n| !is_safe_hidden_name(n)) {
        return Err(io::Error::other("unsafe path component"));
    }
    let mut dir = Dir::open_root()?;
    let last = names.len().saturating_sub(1);
    for (i, name) in names.iter().enumerate() {
        dir = dir.open_or_mkdir_priv(name, true, i == last)?;
    }
    Ok(dir)
}

pub fn uid_file_name(uid: &str) -> io::Result<String> {
    if uid.len() > 80 || uid.is_empty() {
        return Err(io::Error::other("bad uid"));
    }
    if !uid
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(io::Error::other("bad uid"));
    }
    if uid == "." || uid == ".." {
        return Err(io::Error::other("bad uid"));
    }
    Ok(format!("{uid}.vcf"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uid_names() {
        assert!(uid_file_name("a1b2-c3").is_ok());
        assert!(uid_file_name("../etc").is_err());
        assert!(uid_file_name("a/b").is_err());
        assert!(uid_file_name("").is_err());
    }
}
