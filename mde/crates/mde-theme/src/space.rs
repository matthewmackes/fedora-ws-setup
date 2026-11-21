//! 4 px spacing grid. Named aliases stay unset until a measured spec fills them.

pub fn grid(step: u32) -> f32 {
    (step.clamp(1, 8) * 4) as f32
}

pub fn alias(name: &str) -> Result<f32, String> {
    let text = std::fs::read_to_string(crate::color::token_path("space.toml"))
        .map_err(|_| "space.toml missing".to_string())?;
    let value: toml::Value = toml::from_str(&text).map_err(|e| e.to_string())?;
    let status = value
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unmeasured");
    if crate::fidelity_active() && status != "measured" {
        return Err(format!("space alias {name} blocked; space.toml is {status}"));
    }
    let n = value
        .get("alias")
        .and_then(|v| v.get(name))
        .and_then(|v| v.as_float())
        .ok_or_else(|| format!("space alias {name} unset"))? as f32;
    if crate::fidelity_active() && n <= 0.0 {
        return Err(format!("space alias {name} is unset"));
    }
    Ok(n)
}
