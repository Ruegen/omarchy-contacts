use std::io::{self, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

pub const SERVICE: &str = "omarchy-contacts";
pub const ACCOUNT: &str = "icloud";

/// Apple app-specific passwords are 16 letters/digits, often shown in four groups.
pub fn is_app_specific_password(s: &str) -> bool {
    let compact: String = s.chars().filter(|c| *c != '-' && !c.is_whitespace()).collect();
    compact.len() == 16 && compact.chars().all(|c| c.is_ascii_alphanumeric())
}

pub fn reject_regular_password(s: &str) -> Result<(), String> {
    if s.contains('\n') || s.contains('\r') || s.contains('\0') {
        return Err("invalid password".into());
    }
    if s.contains('@') || s.len() > 32 || s.len() < 16 {
        return Err("iCloud needs an app-specific password, not your Apple ID password".into());
    }
    if !is_app_specific_password(s) {
        return Err("iCloud needs an app-specific password from appleid.apple.com".into());
    }
    Ok(())
}

pub fn store_password(password: &str) -> io::Result<()> {
    reject_regular_password(password).map_err(io::Error::other)?;
    let mut child = Command::new("/usr/bin/secret-tool")
        .args(["store", "--label=Omarchy Contacts iCloud", "service", SERVICE, "account", ACCOUNT])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env_remove("SECRET_TOOL_PASSWORD")
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(password.as_bytes())?;
    }
    let st = child.wait()?;
    if !st.success() {
        return Err(io::Error::other("could not store the password in the keyring"));
    }
    Ok(())
}

pub fn load_password() -> io::Result<String> {
    let out = Command::new("/usr/bin/secret-tool")
        .args(["lookup", "service", SERVICE, "account", ACCOUNT])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env_remove("SECRET_TOOL_PASSWORD")
        .output()?;
    if !out.status.success() {
        return Err(io::Error::other("no iCloud password in the keyring"));
    }
    if out.stdout.len() > 64 {
        return Err(io::Error::other("password too large"));
    }
    let s = String::from_utf8(out.stdout).map_err(|_| io::Error::other("password not utf8"))?;
    let s = s.trim_end_matches(['\n', '\r']).to_string();
    reject_regular_password(&s).map_err(io::Error::other)?;
    Ok(s)
}

pub fn clear_password() -> io::Result<()> {
    let _ = Command::new("/usr/bin/secret-tool")
        .args(["clear", "service", SERVICE, "account", ACCOUNT])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(())
}

pub fn never_trace<F, T>(f: F) -> T
where
    F: FnOnce() -> T,
{
    // secret-tool must not inherit bash xtrace from a parent shell.
    let _ = Command::new("/usr/bin/true").env_remove("SHELLOPTS");
    f()
}

#[allow(dead_code)]
fn _no_exec_secret_in_argv() {
    let _ = Command::new("/usr/bin/true").arg0("true");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_specific_ok() {
        assert!(is_app_specific_password("abcd-efgh-ijkl-mnop"));
        assert!(is_app_specific_password("abcdefghijklmnop"));
        assert!(reject_regular_password("abcd-efgh-ijkl-mnop").is_ok());
    }

    #[test]
    fn regular_password_rejected() {
        assert!(reject_regular_password("hunter2-is-my-apple-id-password").is_err());
        assert!(reject_regular_password("me@icloud.com").is_err());
        assert!(reject_regular_password("short").is_err());
        assert!(reject_regular_password("abcd-efgh-ijkl-mnop\n").is_err());
    }
}
