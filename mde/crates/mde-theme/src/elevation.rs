//! Drop shadows. Values match the current flyout shadow until a spec measures them.

pub struct Shadow {
    pub offset_y: f32,
    pub blur: f32,
    pub alpha: f32,
}

pub fn flyout(dark: bool) -> Shadow {
    read(if dark { "flyout_dark" } else { "flyout_light" }).unwrap_or(Shadow {
        offset_y: 8.0,
        blur: 24.0,
        alpha: if dark { 0.45 } else { 0.28 },
    })
}

pub fn dialog(dark: bool) -> Shadow {
    read(if dark { "dialog_dark" } else { "dialog_light" }).unwrap_or(flyout(dark))
}

fn read(key: &str) -> Option<Shadow> {
    let text = std::fs::read_to_string(crate::color::token_path("elevation.toml")).ok()?;
    let value: toml::Value = toml::from_str(&text).ok()?;
    let table = value.get(key)?.as_table()?;
    Some(Shadow {
        offset_y: table.get("offset_y")?.as_float()? as f32,
        blur: table.get("blur")?.as_float()? as f32,
        alpha: table.get("alpha")?.as_float()? as f32,
    })
}
