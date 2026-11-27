use anyhow::{Context, Result};
use mde_network_neighbourhood::config::{ensure_config_dirs, ensure_mount_root, Config};
use mde_network_neighbourhood::dbus;
use mde_network_neighbourhood::discovery::cache;
use mde_network_neighbourhood::media;
use mde_network_neighbourhood::mount;
use std::path::Path;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next() {
        if arg == "--mediafs" {
            let id = args.next().context("place id")?;
            let mp = args.next().context("mountpoint")?;
            return media::mount_blocking(&id, Path::new(&mp));
        }
        if arg == "--sync-units" {
            ensure_config_dirs()?;
            ensure_mount_root()?;
            let cfg = Config::load()?;
            for place in &cfg.places {
                let _ = mount::write_systemd_units(place);
            }
            mount::daemon_reload()?;
            for place in &cfg.places {
                if place.automount {
                    let _ = mount::enable_place(place);
                }
            }
            return Ok(());
        }
        if arg == "--scan-once" {
            ensure_config_dirs()?;
            cache::ensure_state_dir()?;
            let cfg = Config::load().unwrap_or_default();
            let cache = cache::run_scan_and_store(&cfg.settings)?;
            println!(
                "scan complete: {} host(s), status={}",
                cache.hosts.len(),
                cache.last_status
            );
            return Ok(());
        }
    }

    ensure_config_dirs()?;
    ensure_mount_root()?;
    let _ = cache::ensure_state_dir();
    cache::reset_stale_scanning_flag();

    // Ensure units exist for configured places on startup.
    if let Ok(cfg) = Config::load() {
        for place in &cfg.places {
            let _ = mount::write_systemd_units(place);
        }
        let _ = mount::daemon_reload();
    }

    tracing::info!("Network Neighbourhood daemon starting on system bus");
    let _conn = dbus::serve_system().await.context("claim system bus name")?;

    // Reachability probe: feeds the Offline badge, last-seen and back-online notices.
    tokio::spawn(async {
        loop {
            let _ = tokio::task::spawn_blocking(|| {
                let Ok(cfg) = Config::load() else { return };
                let mut seen = mde_network_neighbourhood::status::load_seen();
                let now = mde_network_neighbourhood::status::now_secs();
                for place in &cfg.places {
                    let up = mount::place_reachable(place);
                    let (next, t) = mde_network_neighbourhood::status::observe(
                        seen.get(&place.id),
                        up,
                        now,
                    );
                    match t {
                        mde_network_neighbourhood::status::Transition::WentOffline => {
                            tracing::warn!(place = %place.name, host = %place.host, "place went offline")
                        }
                        mde_network_neighbourhood::status::Transition::CameBack => {
                            tracing::info!(place = %place.name, host = %place.host, "place is back online")
                        }
                        _ => {}
                    }
                    seen.insert(place.id.clone(), next);
                }
                seen.retain(|id, _| cfg.places.iter().any(|p| &p.id == id));
                mde_network_neighbourhood::status::save_seen(&seen);
            })
            .await;
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });

    // Background LAN discovery — keeps /var/lib/network-neighbourhood/discovered.json fresh.
    tokio::spawn(async {
        // Small delay so D-Bus is fully claimed before the first heavy scan.
        tokio::time::sleep(Duration::from_secs(3)).await;
        loop {
            let (enabled, interval) = Config::load()
                .map(|c| {
                    (
                        c.settings.discovery_background,
                        c.settings.discovery_interval_sec.max(60),
                    )
                })
                .unwrap_or((true, 300));

            if enabled {
                tracing::info!("Background discovery pass starting");
                let settings = Config::load()
                    .map(|c| c.settings)
                    .unwrap_or_default();
                match tokio::task::spawn_blocking(move || cache::run_scan_and_store(&settings)).await
                {
                    Ok(Ok(c)) => {
                        tracing::info!(
                            "Background discovery finished: {} host(s) — {}",
                            c.hosts.len(),
                            c.last_status
                        );
                    }
                    Ok(Err(e)) => tracing::warn!("Background discovery failed: {e}"),
                    Err(e) => tracing::warn!("Background discovery join error: {e}"),
                }
            } else {
                tracing::debug!("Background discovery disabled in settings");
            }

            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    });

    std::future::pending::<()>().await;
    Ok(())
}
