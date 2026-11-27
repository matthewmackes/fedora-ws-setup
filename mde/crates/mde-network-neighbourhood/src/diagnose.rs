//! Turn raw mount / ssh / systemd failure text into a plain-language cause and fix.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// Host offline, wrong address, firewall, closed port, DNS.
    Network,
    /// Wrong password, key rejected, account locked.
    Auth,
    /// Package missing, polkit denied, FUSE not permitted.
    Software,
    /// SSH host key changed since last connection.
    HostKey,
    #[default]
    Unknown,
}

/// A one-click remedy the UI can offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "action", content = "arg")]
pub enum AutoFix {
    InstallPackage(String),
    TrustNewHostKey,
    ReenterPassword,
    Retry,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Diagnosis {
    pub kind: ErrorKind,
    pub cause: String,
    pub fix: String,
    pub auto_fix: Option<AutoFix>,
}

fn d(kind: ErrorKind, cause: &str, fix: &str, auto_fix: Option<AutoFix>) -> Diagnosis {
    Diagnosis {
        kind,
        cause: cause.into(),
        fix: fix.into(),
        auto_fix,
    }
}

/// Classify raw error text. Order matters: most specific patterns first.
pub fn classify(raw: &str) -> Diagnosis {
    let t = raw.to_ascii_lowercase();
    let has = |needle: &str| t.contains(needle);

    if has("remote host identification has changed") || has("host key verification failed") {
        return d(
            ErrorKind::HostKey,
            "The host's SSH identity has changed since you last connected.",
            "If you expect this (reinstall, new device), trust the new key. Otherwise do not connect.",
            Some(AutoFix::TrustNewHostKey),
        );
    }
    if has("permission denied (publickey")
        || has("permission denied, please try again")
        || has("permission denied")
            && (has("password") || has("sshfs") || has("ssh"))
        || has("authentication failed")
        || has("logon failure")
        || has("status_logon_failure")
        || has("mount error(13)")
    {
        return d(
            ErrorKind::Auth,
            "The host rejected the username or password.",
            "Re-enter the password (check caps lock and the account name) and retry.",
            Some(AutoFix::ReenterPassword),
        );
    }
    if has("sshfs: command not found")
        || has("mount.fuse.sshfs")
            && (has("not found") || has("no such file"))
        || has("spawn sshfs")
    {
        return d(
            ErrorKind::Software,
            "sshfs is not installed.",
            "Install the fuse-sshfs package.",
            Some(AutoFix::InstallPackage("fuse-sshfs".into())),
        );
    }
    if has("mount.cifs") && (has("not found") || has("no such file")) {
        return d(
            ErrorKind::Software,
            "The SMB mount helper is not installed.",
            "Install the cifs-utils package.",
            Some(AutoFix::InstallPackage("cifs-utils".into())),
        );
    }
    if has("mount.nfs") && (has("not found") || has("no such file")) {
        return d(
            ErrorKind::Software,
            "The NFS mount helper is not installed.",
            "Install the nfs-utils package.",
            Some(AutoFix::InstallPackage("nfs-utils".into())),
        );
    }
    if has("mount.davfs") && (has("not found") || has("no such file")) {
        return d(
            ErrorKind::Software,
            "The WebDAV mount helper is not installed.",
            "Install the davfs2 package.",
            Some(AutoFix::InstallPackage("davfs2".into())),
        );
    }
    if has("fusermount") && has("permission denied")
        || has("allow_other")
        || has("/dev/fuse")
    {
        return d(
            ErrorKind::Software,
            "FUSE is not allowed to share this mount with other users.",
            "Enable `user_allow_other` in /etc/fuse.conf and retry.",
            None,
        );
    }
    if has("authentication is required")
        || has("not authorized")
        || has("administrator authentication failed")
        || has("polkit")
    {
        return d(
            ErrorKind::Software,
            "Administrator permission was not granted.",
            "Retry and approve the authentication prompt.",
            Some(AutoFix::Retry),
        );
    }
    if has("timed out") || has("timeout") || has("hung") {
        return d(
            ErrorKind::Network,
            "The host did not answer in time.",
            "Check the host is on and reachable (VPN, Wi-Fi), then retry.",
            Some(AutoFix::Retry),
        );
    }
    if has("not reachable")
        || has("connection refused")
        || has("no route to host")
        || has("network is unreachable")
        || has("host is down")
    {
        return d(
            ErrorKind::Network,
            "The host cannot be reached on its service port.",
            "Check the address, that the service (SSH/SMB/NFS) is running, and any firewall.",
            Some(AutoFix::Retry),
        );
    }
    if has("name or service not known")
        || has("could not resolve")
        || has("temporary failure in name resolution")
        || has("nodename nor servname")
    {
        return d(
            ErrorKind::Network,
            "The host name could not be resolved.",
            "Check the spelling, or use the IP address instead.",
            None,
        );
    }
    d(
        ErrorKind::Unknown,
        "The mount failed for a reason the app does not recognise.",
        "Open Details and Copy log to see the raw output.",
        Some(AutoFix::Retry),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_password() {
        let x = classify("sshfs exited: mm@172.20.0.2: Permission denied (publickey,password).");
        assert_eq!(x.kind, ErrorKind::Auth);
        assert_eq!(x.auto_fix, Some(AutoFix::ReenterPassword));
    }

    #[test]
    fn host_key_changed() {
        let x = classify("WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!");
        assert_eq!(x.kind, ErrorKind::HostKey);
        assert_eq!(x.auto_fix, Some(AutoFix::TrustNewHostKey));
    }

    #[test]
    fn offline_host() {
        let x = classify("Host 10.0.0.9:22 is not reachable — check VPN/network, then Retry");
        assert_eq!(x.kind, ErrorKind::Network);
    }

    #[test]
    fn missing_sshfs() {
        let x = classify("spawn sshfs: No such file or directory");
        assert_eq!(x.auto_fix, Some(AutoFix::InstallPackage("fuse-sshfs".into())));
    }

    #[test]
    fn timeout() {
        assert_eq!(classify("sshfs timed out after 45s").kind, ErrorKind::Network);
    }

    #[test]
    fn unknown_falls_through() {
        assert_eq!(classify("???").kind, ErrorKind::Unknown);
    }
}
