use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Network place protocol. Tree groups by these labels first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    Smb,
    Ssh,
    Nfs,
    WebDav,
    /// Jellyfin — UI label "Video"
    Video,
    /// Airsonic — UI label "Audio"
    Audio,
}

impl Protocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Smb => "smb",
            Self::Ssh => "ssh",
            Self::Nfs => "nfs",
            Self::WebDav => "webdav",
            Self::Video => "video",
            Self::Audio => "audio",
        }
    }

    /// The standard TCP port for this service.
    pub fn default_port(self) -> u16 {
        match self {
            Self::Smb => 445,
            Self::Ssh => 22,
            Self::Nfs => 2049,
            Self::WebDav => 443,
            Self::Video => 8096,
            Self::Audio => 4040,
        }
    }

    pub fn group_label(self) -> &'static str {
        match self {
            Self::Smb => "SMB",
            Self::Ssh => "SSH",
            Self::Nfs => "NFS",
            Self::WebDav => "WebDAV",
            Self::Video => "Video",
            Self::Audio => "Audio",
        }
    }

    pub fn icon_name(self) -> &'static str {
        match self {
            Self::Smb => "network-server",
            Self::Ssh => "utilities-terminal",
            Self::Nfs => "drive-harddisk",
            Self::WebDav => "network-workgroup",
            Self::Video => "folder-videos",
            Self::Audio => "folder-music",
        }
    }

    pub fn all() -> &'static [Protocol] {
        &[
            Self::Smb,
            Self::Ssh,
            Self::Nfs,
            Self::WebDav,
            Self::Video,
            Self::Audio,
        ]
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "smb" | "cifs" => Some(Self::Smb),
            "ssh" | "sftp" => Some(Self::Ssh),
            "nfs" => Some(Self::Nfs),
            "webdav" | "dav" | "davs" => Some(Self::WebDav),
            "video" | "jellyfin" => Some(Self::Video),
            "audio" | "airsonic" | "navidrome" => Some(Self::Audio),
            _ => None,
        }
    }

    pub fn from_uri_scheme(scheme: &str) -> Option<Self> {
        match scheme.to_ascii_lowercase().as_str() {
            "smb" => Some(Self::Smb),
            "ssh" | "sftp" => Some(Self::Ssh),
            "nfs" => Some(Self::Nfs),
            "dav" | "davs" | "webdav" => Some(Self::WebDav),
            "jellyfin" | "video" => Some(Self::Video),
            "airsonic" | "audio" => Some(Self::Audio),
            "http" | "https" => None,
            _ => None,
        }
    }
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.group_label())
    }
}

/// When to bring the real local mount online.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MountPolicy {
    #[default]
    MountNow,
    MountWhenOpened,
    Automount,
}

impl MountPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MountNow => "mount_now",
            Self::MountWhenOpened => "mount_when_opened",
            Self::Automount => "automount",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::MountNow => "Mount now",
            Self::MountWhenOpened => "Mount when opened",
            Self::Automount => "Automount in background",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "mount_now" | "now" => Some(Self::MountNow),
            "mount_when_opened" | "opened" => Some(Self::MountWhenOpened),
            "automount" | "lazy" => Some(Self::Automount),
            _ => None,
        }
    }

    pub fn all() -> &'static [MountPolicy] {
        &[Self::MountNow, Self::MountWhenOpened, Self::Automount]
    }
}

/// Where secrets are stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CredentialScope {
    #[default]
    Machine,
    User,
}

impl CredentialScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Machine => "machine",
            Self::User => "user",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Machine => "This machine (system)",
            Self::User => "This user only",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "machine" | "system" => Some(Self::Machine),
            "user" => Some(Self::User),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Place {
    pub id: String,
    pub name: String,
    pub protocol: Protocol,
    pub host: String,
    #[serde(default)]
    pub port: Option<u16>,
    /// Share name (SMB), export path (NFS), remote path (SSH/WebDAV), or library root.
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub username: String,
    /// Relative to CREDS_DIR or absolute for user scope.
    #[serde(default)]
    pub credential_file: String,
    #[serde(default)]
    pub credential_scope: CredentialScope,
    /// Optional drive-letter style alias: "Z", "N", …
    #[serde(default)]
    pub drive_letter: Option<String>,
    /// Fedora-standard systemd automount when true (derived from mount_policy too).
    #[serde(default = "default_true")]
    pub automount: bool,
    #[serde(default)]
    pub mount_policy: MountPolicy,
    /// Extra mount options.
    #[serde(default)]
    pub options: String,
    /// Full URL for Video/Audio (Jellyfin/Airsonic) or WebDAV.
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub comment: String,
    /// Last mount error (UI / status).
    #[serde(default)]
    pub last_error: String,
}

fn default_true() -> bool {
    true
}

impl Place {
    pub fn new(name: impl Into<String>, protocol: Protocol, host: impl Into<String>) -> Self {
        let id = Uuid::new_v4().to_string();
        Self {
            id: id.clone(),
            name: name.into(),
            protocol,
            host: host.into(),
            port: None,
            path: String::new(),
            username: String::new(),
            credential_file: format!("{id}.cred"),
            credential_scope: CredentialScope::Machine,
            drive_letter: None,
            automount: true,
            mount_policy: MountPolicy::Automount,
            options: String::new(),
            url: String::new(),
            comment: String::new(),
            last_error: String::new(),
        }
    }

    pub fn sync_automount_flag(&mut self) {
        self.automount = matches!(
            self.mount_policy,
            MountPolicy::Automount | MountPolicy::MountWhenOpened
        );
    }

    /// Absolute mount directory under `/mnt/network/<Group>/<host>/<name>`.
    pub fn mount_point(&self) -> String {
        let group = self.protocol.group_label();
        let host = sanitize_path_component(&self.host);
        let leaf = sanitize_path_component(if self.name.is_empty() {
            &self.path
        } else {
            &self.name
        });
        format!("/mnt/network/{group}/{host}/{leaf}")
    }

    pub fn drive_symlink_path(&self) -> Option<String> {
        self.drive_letter.as_ref().map(|letter| {
            let letter = letter.trim_end_matches(':').to_uppercase();
            format!("/mnt/network/{letter}:")
        })
    }
}

fn sanitize_path_component(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '\0' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string()
}

/// Discovered (not yet saved) host/share.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredHost {
    pub name: String,
    pub host: String,
    pub protocol: Protocol,
    pub port: Option<u16>,
    pub path: String,
    pub url: String,
    pub reachable: bool,
    pub source: String,
    /// Unix epoch seconds when this host was last observed in a scan.
    #[serde(default)]
    pub last_seen: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_point_sanitises() {
        let mut p = Place::new("My Share", Protocol::Smb, "NAS.local");
        p.path = "media".to_string();
        assert_eq!(p.mount_point(), "/mnt/network/SMB/NAS.local/My Share");
    }
}
