use crate::place::{CredentialScope, MountPolicy, Place};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub const CONFIG_DIR: &str = "/etc/network-neighbourhood";
pub const CREDS_DIR: &str = "/etc/network-neighbourhood/creds";
pub const MOUNT_ROOT: &str = "/mnt/network";
pub const PLACES_FILE: &str = "/etc/network-neighbourhood/places.json";
pub const SETTINGS_FILE: &str = "/etc/network-neighbourhood/settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_true")]
    pub discovery_classic: bool,
    #[serde(default = "default_true")]
    pub discovery_scan: bool,
    #[serde(default)]
    pub scan_subnets: Vec<String>,
    #[serde(default = "default_ports")]
    pub scan_ports: Vec<u16>,
    #[serde(default)]
    pub default_mount_policy: MountPolicy,
    #[serde(default)]
    pub default_credential_scope: CredentialScope,
    /// systemd mount TimeoutSec
    #[serde(default = "default_timeout")]
    pub mount_timeout_sec: u64,
    /// automount TimeoutIdleSec
    #[serde(default = "default_idle")]
    pub automount_idle_sec: u64,
    /// Extra options appended for each protocol key (smb, ssh, …).
    #[serde(default)]
    pub mount_option_presets: HashMap<String, String>,
    /// compact | comfortable | dense
    #[serde(default = "default_density")]
    pub ui_density: String,
    /// action id → key name (e.g. "refresh" → "F5")
    #[serde(default = "default_shortcuts")]
    pub shortcuts: HashMap<String, String>,
    /// Timed admin session length in seconds (GUI hint; polkit keep handles real auth).
    #[serde(default = "default_admin_session")]
    pub admin_session_sec: u64,
    /// Continuously scan the LAN in network-neighbourhoodd and cache results.
    #[serde(default = "default_true")]
    pub discovery_background: bool,
    /// Seconds between background discovery passes (minimum enforced at 60).
    #[serde(default = "default_discovery_interval")]
    pub discovery_interval_sec: u64,
}

fn default_true() -> bool {
    true
}
fn default_timeout() -> u64 {
    30
}
fn default_idle() -> u64 {
    600
}
fn default_density() -> String {
    "comfortable".into()
}
fn default_admin_session() -> u64 {
    300
}
fn default_discovery_interval() -> u64 {
    300
}
fn default_ports() -> Vec<u16> {
    vec![22, 111, 139, 445, 2049, 4040, 8096, 8920]
}
fn default_shortcuts() -> HashMap<String, String> {
    HashMap::from([
        ("refresh".into(), "F5".into()),
        ("add".into(), "<Control>n".into()),
        ("disconnect".into(), "<Control>d".into()),
        ("open".into(), "<Control>o".into()),
        ("properties".into(), "<Alt>Return".into()),
        ("discover".into(), "<Control>f".into()),
        ("settings".into(), "<Control>comma".into()),
    ])
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            discovery_classic: true,
            discovery_scan: true,
            scan_subnets: vec![],
            scan_ports: default_ports(),
            default_mount_policy: MountPolicy::MountNow,
            default_credential_scope: CredentialScope::Machine,
            mount_timeout_sec: default_timeout(),
            automount_idle_sec: default_idle(),
            mount_option_presets: HashMap::new(),
            ui_density: default_density(),
            shortcuts: default_shortcuts(),
            admin_session_sec: default_admin_session(),
            discovery_background: true,
            discovery_interval_sec: default_discovery_interval(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub places: Vec<Place>,
    #[serde(default)]
    pub settings: Settings,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Path::new(PLACES_FILE);
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path)
            .with_context(|| format!("read {}", path.display()))?;
        let mut cfg: Config = serde_json::from_str(&text)
            .with_context(|| format!("parse {}", path.display()))?;
        let sp = Path::new(SETTINGS_FILE);
        if sp.exists() {
            let st = fs::read_to_string(sp)?;
            if let Ok(settings) = serde_json::from_str::<Settings>(&st) {
                cfg.settings = settings;
            }
        }
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        ensure_config_dirs()?;
        let json = serde_json::to_string_pretty(self)?;
        atomic_write_mode(Path::new(PLACES_FILE), json.as_bytes(), 0o640)?;
        let settings_json = serde_json::to_string_pretty(&self.settings)?;
        atomic_write_mode(Path::new(SETTINGS_FILE), settings_json.as_bytes(), 0o644)?;
        Ok(())
    }

    pub fn find(&self, id: &str) -> Option<&Place> {
        self.places.iter().find(|p| p.id == id)
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut Place> {
        self.places.iter_mut().find(|p| p.id == id)
    }

    pub fn upsert(&mut self, place: Place) {
        if let Some(existing) = self.find_mut(&place.id) {
            *existing = place;
        } else {
            self.places.push(place);
        }
    }

    pub fn remove(&mut self, id: &str) -> Option<Place> {
        if let Some(pos) = self.places.iter().position(|p| p.id == id) {
            Some(self.places.remove(pos))
        } else {
            None
        }
    }
}

pub fn ensure_config_dirs() -> Result<()> {
    fs::create_dir_all(CONFIG_DIR)?;
    fs::create_dir_all(CREDS_DIR)?;
    let mut perms = fs::metadata(CREDS_DIR)?.permissions();
    perms.set_mode(0o700);
    fs::set_permissions(CREDS_DIR, perms)?;
    Ok(())
}

pub fn ensure_mount_root() -> Result<()> {
    for group in ["SMB", "SSH", "NFS", "WebDAV", "Video", "Audio"] {
        fs::create_dir_all(PathBuf::from(MOUNT_ROOT).join(group))?;
    }
    Ok(())
}

pub fn user_creds_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("network-neighbourhood")
        .join("creds")
}

pub fn cred_path(place: &Place) -> PathBuf {
    if place.credential_file.starts_with('/') {
        return PathBuf::from(&place.credential_file);
    }
    match place.credential_scope {
        CredentialScope::Machine => PathBuf::from(CREDS_DIR).join(&place.credential_file),
        CredentialScope::User => user_creds_dir().join(&place.credential_file),
    }
}

fn webdav_url(place: &Place) -> String {
    if place.url.is_empty() {
        format!(
            "https://{}/{}",
            place.host,
            place.path.trim_start_matches('/')
        )
    } else {
        place.url.clone()
    }
}

const DAVFS_SECRETS: &str = "/etc/davfs2/secrets";

/// Merge a WebDAV line into `/etc/davfs2/secrets` (davfs2 ignores our per-place cred file).
fn install_davfs_secret(place: &Place, password: &str) -> Result<()> {
    let url = webdav_url(place);
    let line = format!("{url} {} {password}", place.username);
    let existing = if Path::new(DAVFS_SECRETS).exists() {
        fs::read_to_string(DAVFS_SECRETS).unwrap_or_default()
    } else {
        if let Some(parent) = Path::new(DAVFS_SECRETS).parent() {
            fs::create_dir_all(parent)?;
        }
        String::new()
    };
    let mut out = String::new();
    let mut replaced = false;
    for raw in existing.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            out.push_str(raw);
            out.push('\n');
            continue;
        }
        let key = trimmed.split_whitespace().next().unwrap_or("");
        if key == url {
            out.push_str(&line);
            out.push('\n');
            replaced = true;
        } else {
            out.push_str(raw);
            out.push('\n');
        }
    }
    if !replaced {
        out.push_str(&line);
        out.push('\n');
    }
    atomic_write_mode(Path::new(DAVFS_SECRETS), out.as_bytes(), 0o600)?;
    Ok(())
}

fn remove_davfs_secret(place: &Place) -> Result<()> {
    let path = Path::new(DAVFS_SECRETS);
    if !path.exists() {
        return Ok(());
    }
    let url = webdav_url(place);
    let existing = fs::read_to_string(path)?;
    let mut out = String::new();
    let mut changed = false;
    for raw in existing.lines() {
        let trimmed = raw.trim();
        let key = trimmed.split_whitespace().next().unwrap_or("");
        if !trimmed.is_empty() && !trimmed.starts_with('#') && key == url {
            changed = true;
            continue;
        }
        out.push_str(raw);
        out.push('\n');
    }
    if changed {
        atomic_write_mode(path, out.as_bytes(), 0o600)?;
    }
    Ok(())
}

/// Write credential file mode 0600. Format depends on protocol.
pub fn write_credentials(place: &Place, password: &str) -> Result<PathBuf> {
    let path = cred_path(place);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        if place.credential_scope == CredentialScope::User {
            let mut perms = fs::metadata(parent)?.permissions();
            perms.set_mode(0o700);
            fs::set_permissions(parent, perms)?;
        } else {
            ensure_config_dirs()?;
        }
    }
    let body = match place.protocol {
        crate::Protocol::Smb => format!("username={}\npassword={}\n", place.username, password),
        crate::Protocol::WebDav => {
            let url = webdav_url(place);
            // Also install into davfs2 secrets — systemd davfs mounts read that file.
            let _ = install_davfs_secret(place, password);
            format!("{url} {} {password}\n", place.username)
        }
        crate::Protocol::Ssh | crate::Protocol::Video | crate::Protocol::Audio => {
            format!("{password}\n")
        }
        crate::Protocol::Nfs => String::new(),
    };
    if !body.is_empty() {
        atomic_write_mode(&path, body.as_bytes(), 0o600)?;
    }
    Ok(path)
}

/// Remove per-place credential / key files (and davfs2 secret line for WebDAV).
pub fn delete_credentials(place: &Place) -> Result<()> {
    let path = cred_path(place);
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    }
    let key = PathBuf::from(CREDS_DIR).join(format!("{}.key", place.id));
    if key.exists() {
        fs::remove_file(&key).with_context(|| format!("remove {}", key.display()))?;
    }
    if place.protocol == crate::Protocol::WebDav {
        let _ = remove_davfs_secret(place);
    }
    Ok(())
}

pub fn read_password(place: &Place) -> Result<String> {
    let path = cred_path(place);
    if !path.exists() {
        return Ok(String::new());
    }
    let text = fs::read_to_string(&path)?;
    match place.protocol {
        crate::Protocol::Smb => {
            for line in text.lines() {
                if let Some(rest) = line.strip_prefix("password=") {
                    return Ok(rest.to_string());
                }
            }
            Ok(String::new())
        }
        crate::Protocol::WebDav => Ok(text
            .split_whitespace()
            .nth(2)
            .unwrap_or("")
            .to_string()),
        _ => Ok(text.trim().to_string()),
    }
}

fn atomic_write_mode(path: &Path, data: &[u8], mode: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(mode)
            .open(&tmp)
            .with_context(|| format!("open {}", tmp.display()))?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    fs::set_permissions(&tmp, fs::Permissions::from_mode(mode))?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::place::{Place, Protocol};

    #[test]
    fn roundtrip_json() {
        let mut cfg = Config::default();
        cfg.places
            .push(Place::new("test", Protocol::Smb, "host.local"));
        let s = serde_json::to_string(&cfg).unwrap();
        let back: Config = serde_json::from_str(&s).unwrap();
        assert_eq!(back.places.len(), 1);
    }
}
