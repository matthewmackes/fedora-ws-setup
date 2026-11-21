//! mde-comp — MDE Wayland compositor (tabs-only).
//!
//! Product locks: smithay-based, server-side tab strip only, standard protocols,
//! compositor-owned output profiles + fractional scale, greeter stage then handoff.
//!
//! With `--features smithay` (default), `--session` runs the DRM/TTY backend
//! (mde-anvil) and a side-thread control socket.
//!
//! Nested/debug: `cargo run -p mde-comp --features nested -- --nested`
//! (or `MDE_COMP_NESTED=1`) uses a winit window instead of DRM/VT.

mod ctl;
mod greeter;
mod outputs;
mod session;
mod tabs;

use anyhow::Result;
use clap::{Parser, Subcommand};
use mde_config::MdeConfig;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "mde-comp", about = "Mackes Desktop Environment compositor")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
    /// Run as session compositor (default when no subcommand).
    #[arg(long)]
    session: bool,
    /// Control-plane only (no DRM). For smoke tests / tooling.
    #[arg(long)]
    ctl_only: bool,
    /// Nested winit window backend (no DRM/VT). Requires `--features nested`.
    /// Also enabled by `MDE_COMP_NESTED=1`.
    #[arg(long)]
    nested: bool,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    Selftest,
    /// Greeter stage before user session handoff (seatd/greetd).
    Greeter,
    /// Print active output profile JSON from config.
    Outputs,
    /// Talk to a running session control socket.
    Ctl {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() -> Result<()> {
    mde_config::supervisor::install_panic_hook();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.cmd {
        Some(Cmd::Selftest) => {
            let cfg = MdeConfig::default();
            assert_eq!(cfg.active().compositor.layout, "tabs");
            assert!(!cfg.active().compositor.server_tab_strip);
            tabs::selftest()?;
            outputs::selftest(&cfg)?;
            greeter::selftest()?;
            session::selftest()?;
            println!(
                "{{\"ok\":true,\"product\":\"{}\",\"layout\":\"tabs\"}}",
                mde_theme::PRODUCT
            );
            Ok(())
        }
        Some(Cmd::Greeter) => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(greeter::run())
        }
        Some(Cmd::Outputs) => {
            let cfg = MdeConfig::load().unwrap_or_default();
            println!("{}", serde_json::to_string_pretty(&cfg.active().displays)?);
            Ok(())
        }
        Some(Cmd::Ctl { args }) => ctl::run(&args),
        None => {
            let cfg = MdeConfig::load().unwrap_or_default();
            info!(
                "mde-comp {} starting (layout={}, xwayland={})",
                mde_theme::VERSION_FULL,
                cfg.active().compositor.layout,
                cfg.active().session.xwayland
            );
            if cli.ctl_only {
                info!("control-plane only (--ctl-only)");
                let rt = tokio::runtime::Runtime::new()?;
                return rt.block_on(session::run_session());
            }
            if cli.session
                || nested_requested(&cli)
                || std::env::var_os("MDE_COMP_FOREGROUND").is_some()
            {
                run_session_mode(nested_requested(&cli))?;
            } else {
                println!(
                    "{{\"ready\":true,\"socket\":\"{}\"}}",
                    session::socket_path().display()
                );
            }
            Ok(())
        }
    }
}

fn nested_requested(cli: &Cli) -> bool {
    cli.nested
        || std::env::var_os("MDE_COMP_NESTED")
            .map(|v| v != "0" && !v.is_empty() && v != "false")
            .unwrap_or(false)
}

fn run_session_mode(nested: bool) -> Result<()> {
    #[cfg(feature = "smithay")]
    {
        let cfg = MdeConfig::load().unwrap_or_default();
        if cfg.active().session.xwayland {
            std::env::set_var("MDE_XWAYLAND", "1");
            info!("XWayland enabled via session.xwayland");
        } else if std::env::var_os("MDE_XWAYLAND").is_none() {
            std::env::set_var("MDE_XWAYLAND", "0");
        }
        // Control plane on a side thread; DRM/calloop (or winit) owns the main thread.
        std::thread::Builder::new()
            .name("mde-comp-ctl".into())
            .spawn(|| {
                let rt = match tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        tracing::error!("ctl runtime: {e}");
                        return;
                    }
                };
                if let Err(e) = rt.block_on(session::run_session()) {
                    tracing::error!("control socket exited: {e:#}");
                }
            })?;
        if nested {
            #[cfg(feature = "nested")]
            {
                info!("starting nested winit backend (mde-anvil) + control socket");
                mde_anvil::run_winit();
            }
            #[cfg(not(feature = "nested"))]
            {
                anyhow::bail!(
                    "--nested / MDE_COMP_NESTED requires building with `--features nested` \
                     (e.g. cargo run -p mde-comp --features nested -- --nested)"
                );
            }
        } else {
            info!("starting DRM/TTY smithay backend (mde-anvil) + control socket");
            mde_anvil::run_udev();
        }
        // Clear ready marker on exit.
        if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
            let _ = std::fs::remove_file(std::path::Path::new(&runtime).join("mde-comp-wayland"));
        }
        Ok(())
    }
    #[cfg(not(feature = "smithay"))]
    {
        let _ = nested;
        info!("DRM/smithay backend disabled at build time; control-plane only");
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(session::run_session())
    }
}
