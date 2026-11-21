//! Greeter stage: seatd/greetd → authenticate → start mde-session.
//!
//! Speaks greetd IPC when `GREETD_SOCK` is set. With a display, execs the iced
//! `mde-greeter` UI. Without display / JSONL / smoke, keeps the headless path
//! (`{"stage":"greeter","ready":true}` + optional env-based auth).

use anyhow::{bail, Context, Result};
use mde_greeter::greetd;
use std::env;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::Command;
use tracing::info;

pub fn selftest() -> Result<()> {
    let session = env::var("MDE_SESSION_BIN").unwrap_or_else(|_| "mde-session".into());
    assert!(!session.is_empty());
    let _ = greetd::session_command();
    Ok(())
}

pub async fn run() -> Result<()> {
    info!("mde-comp greeter stage ready (seatd/greetd)");
    println!("{{\"stage\":\"greeter\",\"ready\":true}}");

    if let Ok(sock) = env::var("GREETD_SOCK") {
        return run_with_sock(&sock);
    }

    if env::var_os("MDE_GREETER_HANDOFF").is_some() {
        handoff_session()?;
    }
    Ok(())
}

fn run_with_sock(sock_path: &str) -> Result<()> {
    // Graphical path: cage (or similar) sets WAYLAND_DISPLAY; launch iced UI.
    if !greetd::prefer_headless() {
        return exec_graphical_greeter();
    }

    let user = env::var("MDE_GREETER_USER").unwrap_or_default();
    if user.is_empty() {
        info!("GREETD_SOCK set but no display / MDE_GREETER_USER; waiting for UI");
        if env::var_os("MDE_GREETER_WAIT").is_some() {
            let _ = UnixStream::connect(sock_path);
            std::thread::park();
        }
        return Ok(());
    }

    let password = env::var("MDE_GREETER_PASSWORD").unwrap_or_default();
    greetd::authenticate(sock_path, &user, &password)
        .with_context(|| format!("greetd authenticate user={user}"))?;
    info!(%user, "greetd start_session ok");
    Ok(())
}

fn exec_graphical_greeter() -> Result<()> {
    let bin = env::var("MDE_GREETER_BIN").unwrap_or_else(|_| "mde-greeter".into());
    info!(%bin, "greeter → graphical mde-greeter");
    let err = Command::new(&bin).exec();
    bail!("exec {bin} failed: {err}");
}

fn handoff_session() -> Result<()> {
    let bin = env::var("MDE_SESSION_BIN").unwrap_or_else(|_| "mde-session".into());
    info!(%bin, "greeter → user session handoff");
    let status = Command::new(&bin).status()?;
    if !status.success() {
        bail!("session handoff failed: {bin} exited {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selftest_ok() {
        selftest().unwrap();
    }
}
