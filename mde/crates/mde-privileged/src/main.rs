//! `mde-privileged` — system actions authorized through polkit.
//! Prefer existing D-Bus services; this binary covers MDE-specific gaps.

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use std::process::Command;

#[derive(Parser, Debug)]
#[command(name = "mde-privileged", about = "MDE privileged helper (polkit-gated)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Run a self-check (no privileged work).
    Selftest,
    /// Trigger PackageKit offline update prepare (reboot/power-off).
    OfflineTrigger { mode: String },
    /// Ask systemd to reboot / poweroff / suspend (via logind).
    Power { action: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Selftest => {
            println!("{{\"ok\":true,\"service\":\"mde-privileged\"}}");
            Ok(())
        }
        Cmd::OfflineTrigger { mode } => {
            // polkit prompt is owned by PackageKit; we only invoke the well-known API.
            let method = match mode.as_str() {
                "reboot" | "power-off" => mode,
                _ => bail!("mode must be reboot or power-off"),
            };
            let status = Command::new("gdbus")
                .args([
                    "call",
                    "--system",
                    "--dest",
                    "org.freedesktop.PackageKit",
                    "--object-path",
                    "/org/freedesktop/PackageKit",
                    "--method",
                    "org.freedesktop.PackageKit.Offline.Trigger",
                    &method,
                ])
                .status()?;
            if !status.success() {
                bail!("PackageKit Offline.Trigger failed");
            }
            Ok(())
        }
        Cmd::Power { action } => {
            let unit = match action.as_str() {
                "reboot" => "reboot",
                "poweroff" | "shutdown" => "poweroff",
                "suspend" => "suspend",
                _ => bail!("unknown power action"),
            };
            let status = Command::new("systemctl").arg(unit).status()?;
            if !status.success() {
                bail!("systemctl {unit} failed");
            }
            Ok(())
        }
    }
}
