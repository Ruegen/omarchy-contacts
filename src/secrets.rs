use std::io::{self, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

pub const SERVICE: &str = "omarchy-contacts";
pub const ACCOUNT: &str = "icloud";

fn is_dash(c: char) -> bool {
    matches!(
        c,
        '-' | '\u{00ad}' | '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}'
            | '\u{2015}' | '\u{2212}' | '\u{fe58}' | '\u{fe63}' | '\u{ff0d}'
    )
}

/// Apple shows these as four groups of four letters, with or without dashes.
pub fn compact_app_password(s: &str) -> String {
    let line = s.lines().next().unwrap_or("").trim();
    line.chars()
        .filter(|c| !c.is_whitespace() && !is_dash(*c) && *c != '\u{200b}' && *c != '\u{feff}')
        .collect()
}

pub fn is_app_specific_password(s: &str) -> bool {
    let compact = compact_app_password(s);
    compact.len() == 16 && compact.chars().all(|c| c.is_ascii_alphanumeric())
}

pub fn reject_regular_password(s: &str) -> Result<String, String> {
    if s.contains('\0') {
        return Err("invalid password".into());
    }
    if s.contains('@') {
        return Err("Use the app-specific password, not the Apple ID".into());
    }
    let compact = compact_app_password(s);
    if compact.len() != 16 || !compact.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(
            "That is not an app-specific password. Generate one at appleid.apple.com (Sign-In & Security), then paste the 16 letters. Dashes are fine."
                .into(),
        );
    }
    Ok(compact)
}

pub fn store_password(password: &str) -> io::Result<()> {
    let compact = reject_regular_password(password).map_err(io::Error::other)?;
    let mut child = Command::new("/usr/bin/secret-tool")
        .args(["store", "--label=Omarchy Contacts iCloud", "service", SERVICE, "account", ACCOUNT])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .env_remove("SECRET_TOOL_PASSWORD")
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(compact.as_bytes())?;
    }
    let st = child.wait()?;
    if !st.success() {
        return Err(io::Error::other(
            "could not store the password in the keyring (is it unlocked?)",
        ));
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
    reject_regular_password(&s).map_err(io::Error::other)
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
        assert_eq!(compact_app_password("abcd-efgh-ijkl-mnop"), "abcdefghijklmnop");
        assert!(is_app_specific_password("abcd-efgh-ijkl-mnop"));
        assert!(is_app_specific_password("abcdefghijklmnop"));
        assert!(is_app_specific_password(&format!(
            "abcd{}efgh{}ijkl{}mnop",
            '\u{2010}', '\u{2013}', '\u{00a0}'
        )));
        assert!(is_app_specific_password("abcd-efgh-ijkl-mnop\n"));
        assert_eq!(
            reject_regular_password("huue-idya-tiry-tbpc").unwrap(),
            "huueidyatirytbpc"
        );
    }

    #[test]
    fn regular_password_rejected() {
        assert!(reject_regular_password("hunter2-is-my-apple-id-password").is_err());
        assert!(reject_regular_password("me@icloud.com").is_err());
        assert!(reject_regular_password("short").is_err());
    }
}
