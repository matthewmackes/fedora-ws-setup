//! Load surface specs. Numbers stay in the TOML files.

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct SurfaceSpec {
    pub schema: u32,
    pub surface: String,
    pub revision: u32,
    pub status: String,
    #[serde(default)]
    pub reference_build: String,
    #[serde(default)]
    pub dpi_basis: u32,
    #[serde(default)]
    pub capture: Capture,
    #[serde(default)]
    pub states: Vec<State>,
    #[serde(default)]
    pub metrics: Vec<Metric>,
    #[serde(default)]
    pub regions: Vec<Region>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Capture {
    #[serde(default)]
    pub logical: Vec<u32>,
    #[serde(default)]
    pub scales: Vec<f64>,
    #[serde(default)]
    pub themes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct State {
    pub id: String,
    #[serde(default)]
    pub refs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Metric {
    pub id: String,
    #[serde(default)]
    pub value_px: f64,
    #[serde(default)]
    pub tolerance_px: f64,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Region {
    pub id: String,
    #[serde(default)]
    pub rect: Vec<u32>,
    #[serde(default)]
    pub ssim_min: f64,
    #[serde(default)]
    pub mae_max: f64,
    #[serde(default)]
    pub compare: String,
}

pub fn fidelity_root() -> PathBuf {
    if let Ok(root) = std::env::var("MDE_FIDELITY_ROOT") {
        return PathBuf::from(root);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fidelity")
}

pub fn load_surface(path: &Path) -> Result<SurfaceSpec, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn load_named(surface: &str) -> Result<SurfaceSpec, String> {
    load_surface(&fidelity_root().join("specs").join(format!("{surface}.toml")))
}

/// Reference scoring is blocked until a human capture promotes the spec.
pub fn require_measured(spec: &SurfaceSpec) -> Result<(), String> {
    if spec.status != "measured" {
        return Err(format!(
            "{} status is {}; missing captures are listed in CAPTURE.md",
            spec.surface, spec.status
        ));
    }
    Ok(())
}
