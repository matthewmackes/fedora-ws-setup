//! Passwordless SSH setup: generate a per-place key, install it on the host
//! using the saved password, verify it works, and roll back on failure.
//!
//! Keys live under `CREDS_DIR` (root-owned) as `<place id>.key` — the same
//! path `mount::mount_spec` already looks for, so a place with a key switches
//! from password mounting to key mounting automatically.

use crate::config::{read_password, CREDS_DIR};
use crate::place::{Place, Protocol};
use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn key_path(place: &Place) -> PathBuf {
    PathBuf::from(CREDS_DIR).join(format!("{}.key", place.id))
}

pub fn has_key(place: &Place) -> bool {
    key_path(place).exists()
}

fn target(place: &Place) -> String {
    if place.username.is_empty() {
        place.host.clone()
    } else {
        format!("{}@{}", place.username, place.host)
    }
}

pub fn keygen_args(place: &Place) -> Vec<String> {
    vec![
        "-q".into(),
        "-t".into(),
        "ed25519".into(),
        "-N".into(),
        String::new(),
        "-C".into(),
        format!("network-neighbourhood:{}", place.id),
        "-f".into(),
        key_path(place).display().to_string(),
    ]
}

pub fn copy_id_args(place: &Place) -> Vec<String> {
    vec![
        "-i".into(),
        format!("{}.pub", key_path(place).display()),
        "-p".into(),
        place.port.unwrap_or(22).to_string(),
        "-o".into(),
        "StrictHostKeyChecking=accept-new".into(),
        "-o".into(),
        "ConnectTimeout=15".into(),
        target(place),
    ]
}

pub fn verify_args(place: &Place) -> Vec<String> {
    vec![
        "-i".into(),
        key_path(place).display().to_string(),
        "-p".into(),
        place.port.unwrap_or(22).to_string(),
        "-o".into(),
        "BatchMode=yes".into(),
        "-o".into(),
        "IdentitiesOnly=yes".into(),
        "-o".into(),
        "PasswordAuthentication=no".into(),
        "-o".into(),
        "StrictHostKeyChecking=accept-new".into(),
        "-o".into(),
        "ConnectTimeout=15".into(),
        target(place),
        "true".into(),
    ]
}

fn run(cmd: &mut Command, what: &str) -> Result<()> {
    let out = cmd
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("run {what}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("{what} failed: {}", err.trim().chars().take(400).collect::<String>())
    }
}

fn remove_key(place: &Place) {
    let k = key_path(place);
    let _ = std::fs::remove_file(&k);
    let _ = std::fs::remove_file(format!("{}.pub", k.display()));
}

/// Set up key auth for an SSH place. Idempotent: an existing working key is kept.
pub fn setup_ssh_key(place: &Place) -> Result<()> {
    if place.protocol != Protocol::Ssh {
        bail!("key setup only applies to SSH places");
    }
    if has_key(place) {
        if run(Command::new("ssh").args(verify_args(place)), "ssh key check").is_ok() {
            tracing::info!(place = %place.id, "ssh key already installed and working");
            return Ok(());
        }
        remove_key(place);
    }
    let password = read_password(place).unwrap_or_default();
    if password.is_empty() {
        bail!("SSH password is empty — save the password first, then set up keys");
    }

    tracing::info!(place = %place.id, "generating ssh key");
    run(Command::new("ssh-keygen").args(keygen_args(place)), "ssh-keygen")?;

    tracing::info!(place = %place.id, host = %place.host, "installing ssh key on host");
    let installed = run(
        Command::new("sshpass")
            .env("SSHPASS", &password)
            .arg("-e")
            .arg("ssh-copy-id")
            .args(copy_id_args(place)),
        "ssh-copy-id",
    );
    if let Err(e) = installed {
        remove_key(place);
        return Err(e);
    }

    tracing::info!(place = %place.id, "verifying passwordless login");
    if let Err(e) = run(Command::new("ssh").args(verify_args(place)), "key login check") {
        remove_key(place);
        bail!("the host accepted the key but passwordless login still failed (rolled back): {e}");
    }
    tracing::info!(place = %place.id, "ssh key setup complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place() -> Place {
        let mut p = Place::new("box", Protocol::Ssh, "10.0.0.5");
        p.username = "mm".into();
        p.port = Some(2222);
        p
    }

    #[test]
    fn copy_id_targets_user_host_and_port() {
        let a = copy_id_args(&place());
        assert!(a.contains(&"mm@10.0.0.5".to_string()));
        assert!(a.windows(2).any(|w| w == ["-p", "2222"]));
    }

    #[test]
    fn verify_is_non_interactive() {
        let a = verify_args(&place());
        assert!(a.contains(&"BatchMode=yes".to_string()));
        assert!(a.contains(&"PasswordAuthentication=no".to_string()));
        assert_eq!(a.last().unwrap(), "true");
    }

    #[test]
    fn key_lives_in_creds_dir() {
        let p = place();
        assert!(key_path(&p).starts_with(CREDS_DIR));
        assert!(key_path(&p).to_string_lossy().ends_with(".key"));
    }

    #[test]
    fn rejects_non_ssh() {
        let p = Place::new("x", Protocol::Smb, "h");
        assert!(setup_ssh_key(&p).is_err());
    }
}
