//! Corner radii. The numeric constants in `lib.rs` are the fallback when this
//! file is missing. Fidelity mode rejects an unmeasured radius file.

pub fn control() -> f32 {
    read("control").unwrap_or(crate::CONTROL_RADIUS)
}
pub fn overlay() -> f32 {
    read("overlay").unwrap_or(crate::FLYOUT_RADIUS)
}
pub fn window() -> f32 {
    read("window").unwrap_or(crate::CONTROL_RADIUS)
}

fn read(key: &str) -> Option<f32> {
    let text = std::fs::read_to_string(crate::color::token_path("radius.toml")).ok()?;
    let value: toml::Value = toml::from_str(&text).ok()?;
    if crate::fidelity_active() {
        let status = value.get("status").and_then(|v| v.as_str()).unwrap_or("unmeasured");
        if status != "measured" {
            return None;
        }
    }
    value.get(key).and_then(|v| v.as_float()).map(|v| v as f32)
}
