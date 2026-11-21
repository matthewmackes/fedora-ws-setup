//! mde-mmc-agent — authenticated remote helper for MMC (W476).
//!
//! v1 scaffold: local UNIX socket JSON protocol mirroring helper ops with an
//! allowlist. Network transport / mTLS land in follow-up commits.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(name = "mde-mmc-agent", about = "MDE MMC remote agent")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Print agent identity / capability allowlist.
    Selftest,
    /// Run one allowlisted op locally (same surface the socket will expose).
    Exec {
        /// Operation id (systemctl.start, journal.tail, …).
        op: String,
        /// Positional args for the op.
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Listen on a UNIX socket (default `$XDG_RUNTIME_DIR/mde-mmc-agent.sock`).
    Serve {
        #[arg(long)]
        socket: Option<PathBuf>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct Request {
    op: String,
    args: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Response {
    ok: bool,
    message: String,
}

const ALLOWLIST: &[&str] = &[
    "systemctl.start",
    "systemctl.stop",
    "systemctl.restart",
    "systemctl.enable",
    "systemctl.disable",
    "journal.tail",
    "timers.list",
    "disks.lsblk",
    "packages.rpmqa",
    "selinux.getenforce",
    "selftest",
];

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Selftest => {
            let body = serde_json::json!({
                "ok": true,
                "service": "mde-mmc-agent",
                "allowlist": ALLOWLIST,
            });
            println!("{body}");
            Ok(())
        }
        Cmd::Exec { op, args } => {
            let resp = exec_op(&op, &args)?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            if resp.ok {
                Ok(())
            } else {
                bail!("{}", resp.message)
            }
        }
        Cmd::Serve { socket } => {
            let path = socket.unwrap_or_else(default_socket);
            if path.exists() {
                let _ = std::fs::remove_file(&path);
            }
            let listener = std::os::unix::net::UnixListener::bind(&path)
                .with_context(|| format!("bind {}", path.display()))?;
            tracing::info!("mde-mmc-agent listening on {}", path.display());
            for stream in listener.incoming() {
                let mut stream = stream?;
                if let Err(e) = handle_client(&mut stream) {
                    tracing::warn!("client error: {e:#}");
                }
            }
            Ok(())
        }
    }
}

fn default_socket() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("mde-mmc-agent.sock")
}

fn handle_client(stream: &mut std::os::unix::net::UnixStream) -> Result<()> {
    use std::io::{BufRead, BufReader, Write};
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let req: Request = serde_json::from_str(line.trim()).unwrap_or(Request {
        op: String::new(),
        args: Vec::new(),
    });
    let resp = exec_op(&req.op, &req.args).unwrap_or_else(|e| Response {
        ok: false,
        message: e.to_string(),
    });
    let out = serde_json::to_string(&resp)? + "\n";
    stream.write_all(out.as_bytes())?;
    Ok(())
}

fn exec_op(op: &str, args: &[String]) -> Result<Response> {
    if !ALLOWLIST.contains(&op) && op != "selftest" {
        return Ok(Response {
            ok: false,
            message: format!("op not allowlisted: {op}"),
        });
    }
    match op {
        "selftest" => Ok(Response {
            ok: true,
            message: "ok".into(),
        }),
        "systemctl.start" | "systemctl.stop" | "systemctl.restart" | "systemctl.enable"
        | "systemctl.disable" => {
            let action = op.rsplit('.').next().unwrap_or("status");
            let unit = args.first().context("unit required")?;
            let st = Command::new("systemctl")
                .args([action, unit.as_str()])
                .status()
                .context("systemctl")?;
            Ok(Response {
                ok: st.success(),
                message: format!("systemctl {action} {unit}"),
            })
        }
        "journal.tail" => {
            let n = args.first().map(String::as_str).unwrap_or("50");
            let out = Command::new("journalctl")
                .args(["-n", n, "--no-pager", "-o", "short-iso"])
                .output()
                .context("journalctl")?;
            Ok(Response {
                ok: out.status.success(),
                message: String::from_utf8_lossy(&out.stdout).into(),
            })
        }
        "timers.list" => {
            let out = Command::new("systemctl")
                .args(["list-timers", "--all", "--no-pager"])
                .output()
                .context("list-timers")?;
            Ok(Response {
                ok: out.status.success(),
                message: String::from_utf8_lossy(&out.stdout).into(),
            })
        }
        "disks.lsblk" => {
            let out = Command::new("lsblk")
                .args(["-o", "NAME,SIZE,TYPE,MOUNTPOINT"])
                .output()
                .context("lsblk")?;
            Ok(Response {
                ok: out.status.success(),
                message: String::from_utf8_lossy(&out.stdout).into(),
            })
        }
        "packages.rpmqa" => {
            let out = Command::new("rpm")
                .args(["-qa", "--qf", "%{NAME}\\n"])
                .output()
                .context("rpm")?;
            Ok(Response {
                ok: out.status.success(),
                message: String::from_utf8_lossy(&out.stdout).chars().take(8000).collect(),
            })
        }
        "selinux.getenforce" => {
            let out = Command::new("getenforce").output().context("getenforce")?;
            Ok(Response {
                ok: out.status.success(),
                message: String::from_utf8_lossy(&out.stdout).trim().into(),
            })
        }
        _ => Ok(Response {
            ok: false,
            message: format!("unimplemented {op}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_has_core_ops() {
        assert!(ALLOWLIST.contains(&"journal.tail"));
        assert!(ALLOWLIST.contains(&"systemctl.start"));
    }
}
