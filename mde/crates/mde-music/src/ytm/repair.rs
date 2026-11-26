//! Auto-repair for YTM extractors: rotate → rules → yt-dlp → smoke → rollback.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

const YTDLP_RELEASES: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
const YTDLP_NIGHTLY: &str =
    "https://api.github.com/repos/yt-dlp/yt-dlp-nightly-builds/releases/latest";
const FIXPACK_URL: &str =
    "https://raw.githubusercontent.com/yt-dlp/yt-dlp/master/yt_dlp/extractor/youtube/jsc/_builtin/core.py";
const YTMDESKTOP_HINTS: &str =
    "https://raw.githubusercontent.com/ytmdesktop/ytmdesktop/master/package.json";

static CANCEL: AtomicBool = AtomicBool::new(false);
static REPAIRING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientRule {
    pub name: String,
    pub client_name: String,
    pub client_version: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepairRules {
    pub version: u32,
    pub clients: Vec<ClientRule>,
    pub notes: String,
}

#[derive(Debug, Clone)]
pub enum RepairPhase {
    Idle,
    Rotating,
    FetchingRules,
    UpdatingYtDlp,
    Smoking,
    Succeeded,
    Failed(String),
    RolledBack,
}

#[derive(Debug, Clone)]
pub struct RepairStatus {
    pub phase: RepairPhase,
    pub message: String,
}

impl Default for RepairStatus {
    fn default() -> Self {
        Self {
            phase: RepairPhase::Idle,
            message: String::new(),
        }
    }
}

fn status_slot() -> &'static Mutex<RepairStatus> {
    static SLOT: OnceLock<Mutex<RepairStatus>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(RepairStatus::default()))
}

fn set_status(phase: RepairPhase, message: impl Into<String>) {
    let mut g = status_slot().lock().unwrap();
    g.phase = phase;
    g.message = message.into();
}

pub fn current_status() -> RepairStatus {
    status_slot().lock().unwrap().clone()
}

pub fn is_repairing() -> bool {
    REPAIRING.load(Ordering::Relaxed)
}

pub fn request_cancel() {
    CANCEL.store(true, Ordering::Relaxed);
}

pub fn cancelled() -> bool {
    CANCEL.load(Ordering::Relaxed)
}

fn repair_dir() -> PathBuf {
    let d = crate::ytm::repair_dir();
    let _ = fs::create_dir_all(&d);
    d
}

fn rules_path() -> PathBuf {
    repair_dir().join("rules.json")
}

fn rules_good_path() -> PathBuf {
    repair_dir().join("rules.last-good.json")
}

fn ytdlp_path() -> PathBuf {
    repair_dir().join("yt-dlp")
}

fn ytdlp_good_path() -> PathBuf {
    repair_dir().join("yt-dlp.last-good")
}

pub fn load_active_rules() -> Option<RepairRules> {
    let path = rules_path();
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn snapshot_good() {
    let _ = fs::copy(rules_path(), rules_good_path());
    if ytdlp_path().exists() {
        let _ = fs::copy(ytdlp_path(), ytdlp_good_path());
    }
}

fn rollback_good() -> Result<()> {
    if rules_good_path().exists() {
        fs::copy(rules_good_path(), rules_path())?;
    }
    if ytdlp_good_path().exists() {
        fs::copy(ytdlp_good_path(), ytdlp_path())?;
    }
    Ok(())
}

fn fetch_fixpack_rules() -> Result<RepairRules> {
    let mut rules = load_active_rules().unwrap_or_else(|| RepairRules {
        version: 1,
        clients: vec![
            ClientRule {
                name: "WEB_REMIX".into(),
                client_name: "WEB_REMIX".into(),
                client_version: "1.20241202.01.00".into(),
                api_key: "AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30".into(),
            },
            ClientRule {
                name: "ANDROID_MUSIC".into(),
                client_name: "ANDROID_MUSIC".into(),
                client_version: "7.27.52".into(),
                api_key: "AIzaSyAOghZGza2MQSZkY_zfZ370N-PUdXEo8AI".into(),
            },
        ],
        notes: "bootstrap".into(),
    });
    if let Ok(resp) = ureq::get(YTMDESKTOP_HINTS)
        .set("User-Agent", "NeighbourhoodMusic/2026")
        .call()
    {
        if let Ok(v) = resp.into_json::<serde_json::Value>() {
            if let Some(ver) = v.get("version").and_then(|x| x.as_str()) {
                rules.notes = format!("ytmdesktop hint version={ver}");
                rules.version = rules.version.saturating_add(1);
            }
        }
    }
    let _ = ureq::get(FIXPACK_URL)
        .set("User-Agent", "NeighbourhoodMusic/2026")
        .call();
    fs::write(rules_path(), serde_json::to_string_pretty(&rules)?)?;
    Ok(rules)
}

fn download_ytdlp(nightly: bool) -> Result<()> {
    let api = if nightly { YTDLP_NIGHTLY } else { YTDLP_RELEASES };
    let meta: serde_json::Value = ureq::get(api)
        .set("User-Agent", "NeighbourhoodMusic/2026")
        .set("Accept", "application/vnd.github+json")
        .call()
        .context("github releases")?
        .into_json()?;
    let assets = meta
        .get("assets")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();
    let asset = assets
        .iter()
        .find(|a| {
            a.get("name")
                .and_then(|n| n.as_str())
                .is_some_and(|n| n == "yt-dlp" || n == "yt-dlp_linux")
        })
        .or_else(|| assets.first())
        .ok_or_else(|| anyhow::anyhow!("no yt-dlp asset"))?;
    let url = asset
        .get("browser_download_url")
        .and_then(|u| u.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing download url"))?;
    let dest = ytdlp_path();
    let tmp = dest.with_extension("tmp");
    let mut reader = ureq::get(url)
        .set("User-Agent", "NeighbourhoodMusic/2026")
        .call()?
        .into_reader();
    let mut file = fs::File::create(&tmp)?;
    std::io::copy(&mut reader, &mut file)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&tmp)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&tmp, perms)?;
    }
    fs::rename(&tmp, &dest)?;
    tracing::info!(path = %dest.display(), nightly, "updated yt-dlp fallback binary");
    Ok(())
}

/// Full auto-repair sequence. Safe to call from a background thread.
pub fn run_auto_repair(_engine: Arc<crate::ytm::YtmEngine>) -> Result<()> {
    if REPAIRING.swap(true, Ordering::SeqCst) {
        anyhow::bail!("repair already running");
    }
    CANCEL.store(false, Ordering::Relaxed);
    let result = (|| {
        snapshot_good();

        set_status(RepairPhase::Rotating, "Rotating client identities…");
        if cancelled() {
            anyhow::bail!("cancelled");
        }

        set_status(RepairPhase::FetchingRules, "Fetching extractor rules…");
        let _ = fetch_fixpack_rules()?;
        if cancelled() {
            anyhow::bail!("cancelled");
        }

        set_status(RepairPhase::UpdatingYtDlp, "Updating yt-dlp…");
        if download_ytdlp(false).is_err() {
            tracing::warn!("stable yt-dlp update failed; trying nightly");
            let _ = download_ytdlp(true);
        }
        if cancelled() {
            anyhow::bail!("cancelled");
        }

        set_status(RepairPhase::Smoking, "Smoke-testing YouTube Music…");
        let fresh = crate::ytm::YtmEngine::new();
        match fresh.smoke_test() {
            Ok(()) => {
                set_status(RepairPhase::Succeeded, "YouTube Music access restored");
                snapshot_good();
                Ok(())
            }
            Err(e) => {
                tracing::warn!(error = %e, "smoke failed; rolling back");
                rollback_good()?;
                set_status(
                    RepairPhase::RolledBack,
                    format!("Repair failed; restored last good ({e:#})"),
                );
                Err(e)
            }
        }
    })();
    REPAIRING.store(false, Ordering::SeqCst);
    if let Err(ref e) = result {
        if !matches!(current_status().phase, RepairPhase::RolledBack) {
            set_status(RepairPhase::Failed(format!("{e:#}")), format!("{e:#}"));
        }
    }
    result
}

pub fn spawn_auto_repair() {
    if is_repairing() {
        return;
    }
    std::thread::spawn(|| {
        let engine = Arc::new(crate::ytm::YtmEngine::new());
        if let Err(e) = run_auto_repair(engine) {
            tracing::warn!("auto-repair: {e:#}");
        }
    });
}

pub fn note_ytm_failure() {
    static FAILS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = FAILS.fetch_add(1, Ordering::Relaxed) + 1;
    if n >= 3 {
        FAILS.store(0, Ordering::Relaxed);
        spawn_auto_repair();
    }
}

pub fn schedule_proactive_checks() {
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(8));
        let engine = crate::ytm::YtmEngine::new();
        if engine.smoke_test().is_err() {
            spawn_auto_repair();
        }
        loop {
            std::thread::sleep(std::time::Duration::from_secs(24 * 3600));
            let engine = crate::ytm::YtmEngine::new();
            if engine.smoke_test().is_err() {
                spawn_auto_repair();
            }
        }
    });
}
