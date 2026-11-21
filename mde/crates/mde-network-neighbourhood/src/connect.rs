//! Per-place connect progress for inline Fluent status (daemon ↔ UI).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const CONNECT_DIR: &str = "/var/lib/network-neighbourhood/connect";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConnectState {
    #[default]
    Idle,
    Connecting,
    Connected,
    AlreadyMounted,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectStep {
    pub label: String,
    pub detail: String,
    /// 0.0..=1.0 cumulative progress after this step.
    pub fraction: f64,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectProgress {
    pub id: String,
    pub state: ConnectState,
    pub phrase: String,
    pub steps: Vec<ConnectStep>,
    pub fraction: f64,
    pub mount_point: String,
    pub username: String,
    pub auth_method: String,
    pub error: String,
    pub updated_at: u64,
}

impl ConnectProgress {
    pub fn new(id: &str, mount_point: &str) -> Self {
        Self {
            id: id.to_string(),
            state: ConnectState::Idle,
            phrase: String::new(),
            steps: Vec::new(),
            fraction: 0.0,
            mount_point: mount_point.to_string(),
            username: String::new(),
            auth_method: String::new(),
            error: String::new(),
            updated_at: now_secs(),
        }
    }

    pub fn path_for(id: &str) -> PathBuf {
        Path::new(CONNECT_DIR).join(format!("{id}.json"))
    }

    pub fn load(id: &str) -> Self {
        let path = Self::path_for(id);
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| Self::new(id, ""))
    }

    pub fn save(&self) {
        let _ = fs::create_dir_all(CONNECT_DIR);
        let path = Self::path_for(&self.id);
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }

    pub fn clear_file(id: &str) {
        let _ = fs::remove_file(Self::path_for(id));
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cancel_map() -> &'static Mutex<HashMap<String, AtomicBool>> {
    static MAP: OnceLock<Mutex<HashMap<String, AtomicBool>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn begin_connect(id: &str) {
    let mut map = cancel_map().lock().unwrap();
    map.insert(id.to_string(), AtomicBool::new(false));
}

pub fn request_cancel(id: &str) -> bool {
    let map = cancel_map().lock().unwrap();
    if let Some(flag) = map.get(id) {
        flag.store(true, Ordering::SeqCst);
        true
    } else {
        // No in-flight mount — still mark progress cancelled for UI.
        false
    }
}

pub fn is_cancelled(id: &str) -> bool {
    let map = cancel_map().lock().unwrap();
    map.get(id)
        .map(|f| f.load(Ordering::SeqCst))
        .unwrap_or(false)
}

pub fn end_connect(id: &str) {
    let mut map = cancel_map().lock().unwrap();
    map.remove(id);
}

/// Reporter used by mount helpers to stream verbose protocol steps.
pub struct ProgressReporter {
    pub progress: ConnectProgress,
}

impl ProgressReporter {
    pub fn start(id: &str, mount_point: &str) -> Self {
        begin_connect(id);
        let mut progress = ConnectProgress::new(id, mount_point);
        progress.state = ConnectState::Connecting;
        progress.phrase = "Connecting…".into();
        progress.fraction = 0.02;
        progress.updated_at = now_secs();
        progress.save();
        Self { progress }
    }

    pub fn set_auth(&mut self, username: &str, auth_method: &str) {
        self.progress.username = username.to_string();
        self.progress.auth_method = auth_method.to_string();
        self.progress.updated_at = now_secs();
        self.progress.save();
    }

    pub fn step(&mut self, label: &str, detail: &str, fraction: f64) -> Result<(), Cancelled> {
        if is_cancelled(&self.progress.id) {
            self.mark_cancelled();
            return Err(Cancelled);
        }
        self.progress.steps.push(ConnectStep {
            label: label.to_string(),
            detail: redact(detail),
            fraction,
            ok: true,
        });
        self.progress.fraction = fraction.clamp(0.0, 1.0);
        self.progress.phrase = label.to_string();
        self.progress.updated_at = now_secs();
        self.progress.save();
        if is_cancelled(&self.progress.id) {
            self.mark_cancelled();
            return Err(Cancelled);
        }
        Ok(())
    }

    pub fn fail_step(&mut self, label: &str, detail: &str) {
        self.progress.steps.push(ConnectStep {
            label: label.to_string(),
            detail: redact(detail),
            fraction: self.progress.fraction,
            ok: false,
        });
        self.progress.state = ConnectState::Failed;
        self.progress.phrase = format!("Failed — {label}");
        self.progress.error = redact(detail);
        self.progress.updated_at = now_secs();
        self.progress.save();
        end_connect(&self.progress.id);
    }

    pub fn succeed(&mut self, phrase: &str) {
        self.progress.state = ConnectState::Connected;
        self.progress.phrase = phrase.to_string();
        self.progress.fraction = 1.0;
        self.progress.error.clear();
        self.progress.updated_at = now_secs();
        self.progress.save();
        end_connect(&self.progress.id);
    }

    pub fn already_mounted(&mut self) {
        self.progress.state = ConnectState::AlreadyMounted;
        self.progress.phrase = format!("Already mounted — {}", self.progress.mount_point);
        self.progress.fraction = 1.0;
        self.progress.steps.clear();
        self.progress.updated_at = now_secs();
        self.progress.save();
        end_connect(&self.progress.id);
    }

    pub fn mark_cancelled(&mut self) {
        self.progress.state = ConnectState::Cancelled;
        self.progress.phrase = "Cancelled".into();
        self.progress.error = "Connect cancelled — cleaning up".into();
        self.progress.updated_at = now_secs();
        self.progress.save();
        end_connect(&self.progress.id);
    }
}

#[derive(Debug)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("connect cancelled")
    }
}

impl std::error::Error for Cancelled {}

/// Redact password-like tokens while keeping usernames / auth method text.
pub fn redact(s: &str) -> String {
    let mut out = s.to_string();
    // password=… / passwd=… / -p SECRET patterns
    for key in ["password=", "passwd=", "pwd="] {
        if let Some(idx) = out.to_ascii_lowercase().find(key) {
            let start = idx + key.len();
            let rest = &out[start..];
            let end = rest
                .find(|c: char| c.is_whitespace() || c == ',' || c == '"' || c == '\'')
                .map(|i| start + i)
                .unwrap_or(out.len());
            out.replace_range(start..end, "***");
        }
    }
    // sshpass -p …
    if let Some(idx) = out.find("sshpass -p ") {
        let start = idx + "sshpass -p ".len();
        let rest = &out[start..];
        let end = rest
            .find(char::is_whitespace)
            .map(|i| start + i)
            .unwrap_or(out.len());
        out.replace_range(start..end, "***");
    }
    // user%password smb style
    if let Some(pct) = out.find('%') {
        let after = &out[pct + 1..];
        if !after.is_empty()
            && after
                .chars()
                .next()
                .map(|c| !c.is_whitespace())
                .unwrap_or(false)
        {
            let end = after
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .map(|i| pct + 1 + i)
                .unwrap_or(out.len());
            out.replace_range(pct + 1..end, "***");
        }
    }
    out
}
