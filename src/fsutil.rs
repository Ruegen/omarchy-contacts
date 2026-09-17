//! Open-once path helpers. Every read and write goes through a descriptor
//! that was opened with `O_NOFOLLOW` after the name was checked.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

use libc::{
    fstat, fsync, mkdirat, open, openat, renameat, unlinkat, AT_FDCWD, AT_REMOVEDIR, O_CLOEXEC,
    O_CREAT, O_DIRECTORY, O_EXCL, O_NOFOLLOW, O_NONBLOCK, O_RDONLY, O_RDWR, S_IFDIR, S_IFMT,
    S_IFREG,
};

pub fn current_uid() -> u32 {
    unsafe { libc::geteuid() }
}

pub fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
        && !name.starts_with('.')
}

pub fn is_safe_hidden_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
}

#[derive(Debug)]
pub struct Dir {
    fd: OwnedFd,
}

impl Dir {
    pub fn open_path(path: &Path) -> io::Result<Self> {
        if !path.is_absolute() {
            return err("path must be absolute");
        }
        let mut dir: Option<Dir> = None;
        for comp in path.components() {
            match comp {
                std::path::Component::RootDir => {
                    dir = Some(Self::open_root()?);
                }
                std::path::Component::Normal(name) => {
                    let name = name.to_str().ok_or_else(|| io::Error::other("bad name"))?;
                    if !is_safe_hidden_name(name) && name != ".." {
                        // Allow walking existing home components including hidden ones
                        // only when they are a single path segment.
                    }
                    if name.contains('\0') || name.contains('/') {
                        return err("bad path component");
                    }
                    let parent = dir.take().ok_or_else(|| io::Error::other("no root"))?;
                    dir = Some(parent.open_or_mkdir_priv(name, false, false)?);
                }
                _ => return err("unsupported path component"),
            }
        }
        dir.ok_or_else(|| io::Error::other("empty path"))
    }

    pub fn open_root() -> io::Result<Self> {
        let fd = unsafe { open(b"/\0".as_ptr() as *const _, O_RDONLY | O_DIRECTORY | O_CLOEXEC) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            fd: unsafe { OwnedFd::from_raw_fd(fd) },
        })
    }

    pub fn open_or_mkdir(&self, name: &str, create: bool) -> io::Result<Dir> {
        self.open_or_mkdir_priv(name, create, create)
    }

    pub fn open_or_mkdir_priv(&self, name: &str, create: bool, private: bool) -> io::Result<Dir> {
        if !is_safe_hidden_name(name) {
            return err("unsafe directory name");
        }
        let cname = cstr(name)?;
        let fd = unsafe {
            openat(
                self.fd.as_raw_fd(),
                cname.as_ptr(),
                O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK,
            )
        };
        if fd >= 0 {
            let dir = Self {
                fd: unsafe { OwnedFd::from_raw_fd(fd) },
            };
            if private {
                dir.require_owned_dir()?;
            } else {
                dir.require_dir()?;
            }
            return Ok(dir);
        }
        let errn = io::Error::last_os_error();
        if !create || errn.raw_os_error() != Some(libc::ENOENT) {
            return Err(errn);
        }
        let rc = unsafe { mkdirat(self.fd.as_raw_fd(), cname.as_ptr(), 0o700) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = unsafe {
            openat(
                self.fd.as_raw_fd(),
                cname.as_ptr(),
                O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let dir = Self {
            fd: unsafe { OwnedFd::from_raw_fd(fd) },
        };
        dir.require_owned_dir()?;
        Ok(dir)
    }

    fn require_dir(&self) -> io::Result<()> {
        let st = fstat_fd(self.fd.as_raw_fd())?;
        if st.st_mode & S_IFMT != S_IFDIR {
            return err("not a directory");
        }
        Ok(())
    }

    fn require_owned_dir(&self) -> io::Result<()> {
        self.require_dir()?;
        let st = fstat_fd(self.fd.as_raw_fd())?;
        if st.st_uid != current_uid() {
            return err("directory owner");
        }
        let mode = st.st_mode & 0o777;
        if mode != 0o700 {
            let perm = std::fs::Permissions::from_mode(0o700);
            // fchmod on the held descriptor, not the path.
            let rc = unsafe { libc::fchmod(self.fd.as_raw_fd(), 0o700) };
            if rc != 0 {
                let _ = perm;
                return err("directory mode");
            }
        }
        Ok(())
    }

    pub fn open_read(&self, name: &str) -> io::Result<File> {
        self.open_file(name, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK, 0, false)
    }

    pub fn create_excl(&self, name: &str) -> io::Result<File> {
        self.open_file(
            name,
            O_RDWR | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC,
            0o600,
            true,
        )
    }

    fn open_file(&self, name: &str, flags: i32, mode: u32, create: bool) -> io::Result<File> {
        if create {
            if !is_safe_hidden_name(name) {
                return err("unsafe file name");
            }
        } else if !is_safe_name(name) && !is_safe_hidden_name(name) {
            return err("unsafe file name");
        }
        let cname = cstr(name)?;
        let fd = unsafe { openat(self.fd.as_raw_fd(), cname.as_ptr(), flags, mode) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let file = unsafe { File::from_raw_fd(fd) };
        require_owned_file(&file)?;
        Ok(file)
    }

    pub fn read_capped(&self, name: &str, max: usize) -> io::Result<Vec<u8>> {
        let mut f = self.open_read(name)?;
        read_capped(&mut f, max)
    }

    pub fn atomic_write(&self, name: &str, bytes: &[u8]) -> io::Result<()> {
        if !is_safe_name(name) && !is_safe_hidden_name(name) {
            return err("unsafe file name");
        }
        let tmp = format!(".{name}.tmp-{}", std::process::id());
        if !is_safe_hidden_name(&tmp) {
            return err("unsafe temp name");
        }
        let mut file = self.create_excl(&tmp)?;
        let rc = unsafe { libc::fchmod(file.as_raw_fd(), 0o600) };
        if rc != 0 {
            let _ = self.unlink(&tmp);
            return err("fchmod");
        }
        if let Err(e) = file.write_all(bytes) {
            let _ = self.unlink(&tmp);
            return Err(e);
        }
        if let Err(e) = file.flush() {
            let _ = self.unlink(&tmp);
            return Err(e);
        }
        let rc = unsafe { fsync(file.as_raw_fd()) };
        if rc != 0 {
            let _ = self.unlink(&tmp);
            return Err(io::Error::last_os_error());
        }
        drop(file);
        let from = cstr(&tmp)?;
        let to = cstr(name)?;
        let rc = unsafe { renameat(self.fd.as_raw_fd(), from.as_ptr(), self.fd.as_raw_fd(), to.as_ptr()) };
        if rc != 0 {
            let _ = self.unlink(&tmp);
            return Err(io::Error::last_os_error());
        }
        let _ = unsafe { fsync(self.fd.as_raw_fd()) };
        Ok(())
    }

    pub fn unlink(&self, name: &str) -> io::Result<()> {
        if !is_safe_hidden_name(name) {
            return err("unsafe file name");
        }
        let cname = cstr(name)?;
        let rc = unsafe { unlinkat(self.fd.as_raw_fd(), cname.as_ptr(), 0) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn list_names(&self) -> io::Result<Vec<String>> {
        // fdopendir consumes the fd on some libc; duplicate first.
        let dup = unsafe { libc::dup(self.fd.as_raw_fd()) };
        if dup < 0 {
            return Err(io::Error::last_os_error());
        }
        let dirp = unsafe { libc::fdopendir(dup) };
        if dirp.is_null() {
            unsafe { libc::close(dup) };
            return Err(io::Error::last_os_error());
        }
        let mut names = Vec::new();
        loop {
            unsafe { *libc::__errno_location() = 0 };
            let ent = unsafe { libc::readdir(dirp) };
            if ent.is_null() {
                break;
            }
            let c_name = unsafe { std::ffi::CStr::from_ptr((*ent).d_name.as_ptr()) };
            let Ok(name) = c_name.to_str() else { continue };
            if name == "." || name == ".." {
                continue;
            }
            names.push(name.to_string());
        }
        unsafe { libc::closedir(dirp) };
        Ok(names)
    }

    pub fn raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

pub fn require_owned_file(file: &File) -> io::Result<()> {
    let st = fstat_fd(file.as_raw_fd())?;
    if st.st_mode & S_IFMT != S_IFREG {
        return err("not a regular file");
    }
    if st.st_uid != current_uid() {
        return err("file owner");
    }
    if st.st_nlink != 1 {
        return err("hard link");
    }
    Ok(())
}

pub fn read_capped(file: &mut File, max: usize) -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    loop {
        let n = file.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        if buf.len().saturating_add(n) > max {
            return err("file too large");
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    Ok(buf)
}

pub fn open_nofollow_abs(path: &Path, max: usize) -> io::Result<Vec<u8>> {
    if !path.is_absolute() {
        return err("path must be absolute");
    }
    let mut opts = OpenOptions::new();
    opts.read(true).custom_flags(O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC);
    let mut file = opts.open(path)?;
    require_owned_file(&file)?;
    read_capped(&mut file, max)
}

pub fn atomic_write_abs(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| io::Error::other("no parent"))?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| io::Error::other("bad name"))?;
    let dir = Dir::open_path(parent)?;
    dir.atomic_write(name, bytes)
}

fn fstat_fd(fd: RawFd) -> io::Result<libc::stat> {
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let rc = unsafe { fstat(fd, &mut st) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(st)
}

fn cstr(name: &str) -> io::Result<std::ffi::CString> {
    std::ffi::CString::new(name).map_err(|_| io::Error::other("nul in name"))
}

fn err<T>(msg: &str) -> io::Result<T> {
    Err(io::Error::other(msg))
}

#[allow(dead_code)]
fn _at_fdcwd() -> RawFd {
    AT_FDCWD
}

#[allow(dead_code)]
fn _rmdir_flag() -> i32 {
    AT_REMOVEDIR
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn names() {
        assert!(is_safe_name("abc-123.vcf"));
        assert!(!is_safe_name(".."));
        assert!(!is_safe_name("a/b"));
        assert!(!is_safe_name(".hidden"));
        assert!(is_safe_hidden_name(".config.toml"));
    }

    #[test]
    fn atomic_write_does_not_follow_symlink() {
        let tmp = std::env::temp_dir().join(format!("omarchy-contacts-fs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o700)).unwrap();
        let victim = tmp.join("victim");
        std::fs::write(&victim, b"must survive").unwrap();
        let dest = tmp.join("cache.json");
        symlink(&victim, &dest).unwrap();
        atomic_write_abs(&dest, b"new").unwrap();
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "must survive");
        assert!(dest.is_file() && !dest.is_symlink());
        assert_eq!(std::fs::read(&dest).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
