//! Temporary Python backend bridge (W456) until snap-ins are pure Rust.
//!
//! Contract: invoke `python3 -c` modules under `/usr/share/mde/ws-mmc` or the
//! source tree `mde/shell/ws-mmc` for read-only inventory helpers when Rust
//! backends are incomplete. Mutations prefer `helper` / native Rust.

use std::path::PathBuf;
use std::process::Command;

pub fn share_dir() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("/usr/share/mde/ws-mmc"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../shell/ws-mmc"),
    ];
    candidates.into_iter().find(|p| p.join("mmc.py").is_file())
}

/// Run a small Python one-liner with sys.path set to the MMC share.
pub fn py_eval(code: &str) -> Result<String, String> {
    let Some(share) = share_dir() else {
        return Err("Python ws-mmc share not installed (bridge unavailable)".into());
    };
    let out = Command::new("python3")
        .arg("-c")
        .arg(format!(
            "import sys; sys.path.insert(0, r'{}'); {}",
            share.display(),
            code
        ))
        .output()
        .map_err(|e| format!("python3: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn available() -> bool {
    share_dir().is_some()
}
