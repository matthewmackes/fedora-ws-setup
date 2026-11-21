//! Compositor-owned output profiles (kanshi replacement) + fractional scale.

use anyhow::Result;
use mde_config::{DisplaysConfig, MdeConfig};
use std::fs;
use std::path::PathBuf;
use tracing::info;

pub fn apply_profile(cfg: &DisplaysConfig, name: &str) -> Result<usize> {
    let Some(profile) = cfg.profiles.iter().find(|p| p.name == name) else {
        anyhow::bail!("unknown display profile {name}");
    };
    for out in &profile.outputs {
        info!(
            output = %out.name,
            enabled = out.enabled,
            mode = %out.mode,
            scale = out.scale,
            x = out.x,
            y = out.y,
            "apply output head"
        );
    }
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        let runtime = PathBuf::from(runtime);
        let _ = fs::write(runtime.join("mde-comp-last-profile"), name);
        let _ = fs::write(runtime.join("mde-output-layout.txt"), format_layout(&profile.outputs));
    }
    Ok(profile.outputs.len())
}

/// Line format: `name x y scale enabled` (`enabled` is 1 or 0).
pub fn format_layout(outputs: &[mde_config::OutputHead]) -> String {
    let mut body = String::new();
    for out in outputs {
        let _ = std::fmt::Write::write_fmt(
            &mut body,
            format_args!(
                "{} {} {} {} {}\n",
                out.name,
                out.x,
                out.y,
                out.scale,
                if out.enabled { 1 } else { 0 }
            ),
        );
    }
    body
}

pub fn selftest(cfg: &MdeConfig) -> Result<()> {
    assert!(cfg.active().displays.allow_fractional_scale);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mde_config::{DisplayProfile, OutputHead};

    #[test]
    fn apply_named_profile() {
        let mut cfg = DisplaysConfig::default();
        cfg.profiles.push(DisplayProfile {
            name: "laptop".into(),
            outputs: vec![OutputHead {
                name: "eDP-1".into(),
                enabled: true,
                mode: "1920x1080".into(),
                scale: 1.5,
                transform: "normal".into(),
                x: 0,
                y: 0,
                refresh_hz: 60.0,
                hdr: false,
            }],
        });
        std::env::remove_var("XDG_RUNTIME_DIR");
        assert_eq!(apply_profile(&cfg, "laptop").unwrap(), 1);
        let text = format_layout(&cfg.profiles[0].outputs);
        assert!(text.starts_with("eDP-1 0 0 1.5 1"));
    }
}
