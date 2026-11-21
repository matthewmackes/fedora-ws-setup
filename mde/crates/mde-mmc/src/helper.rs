//! Privileged mutations via pkexec → `/usr/libexec/mde-mmc-helper`.

use std::path::PathBuf;
use std::process::Command;

pub const HELPER: &str = "/usr/libexec/mde-mmc-helper";
pub const HELPER_FALLBACK: &str = "/usr/libexec/ws-mmc-helper";

pub fn helper_path() -> PathBuf {
    let p = PathBuf::from(HELPER);
    if p.is_file() {
        return p;
    }
    let f = PathBuf::from(HELPER_FALLBACK);
    if f.is_file() {
        return f;
    }
    // Dev tree relative to source (optional).
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../shell/ws-mmc/mde-mmc-helper");
    if dev.is_file() {
        return dev;
    }
    p
}

/// Run helper under pkexec. Returns stdout or error text.
pub fn run(args: &[&str]) -> Result<String, String> {
    let path = helper_path();
    if !path.is_file() {
        return Err(format!(
            "Helper missing at {} (install mde-mmc-helper)",
            path.display()
        ));
    }
    let out = Command::new("pkexec")
        .arg(&path)
        .args(args)
        .output()
        .map_err(|e| format!("pkexec: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if out.status.success() {
        Ok(if stdout.is_empty() {
            "OK".into()
        } else {
            stdout
        })
    } else {
        Err(if stderr.is_empty() {
            format!("helper failed ({})", out.status)
        } else {
            stderr
        })
    }
}

pub fn systemctl(args: &[&str]) -> Result<String, String> {
    let mut v = vec!["systemctl"];
    v.extend_from_slice(args);
    run(&v)
}

pub fn audit(who: &str, action: &str, target: &str, detail: &str) {
    let _ = run(&[
        "audit",
        who,
        action,
        target,
        detail,
    ]);
}
