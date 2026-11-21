//! Type ramp. Sizes are the current product's iced sizes, not a measured
//! Win11 ramp. `status = "unmeasured"` on `type.toml` rejects them in fidelity mode.

pub struct TypeRole {
    pub size_px: f32,
    pub line_px: f32,
    pub weight: &'static str,
    pub font: &'static str,
}

fn role(size: f32, line: f32, weight: &'static str) -> TypeRole {
    TypeRole {
        size_px: size,
        line_px: line,
        weight,
        font: "ui",
    }
}

pub fn caption() -> TypeRole {
    load("caption").unwrap_or_else(|| role(10.0, 12.0, "regular"))
}
pub fn body() -> TypeRole {
    load("body").unwrap_or_else(|| role(12.0, 16.0, "regular"))
}
pub fn body_strong() -> TypeRole {
    load("body_strong").unwrap_or_else(|| role(13.0, 16.0, "semibold"))
}
pub fn subtitle() -> TypeRole {
    load("subtitle").unwrap_or_else(|| role(14.0, 18.0, "semibold"))
}
pub fn title() -> TypeRole {
    load("title").unwrap_or_else(|| role(16.0, 20.0, "semibold"))
}
pub fn display() -> TypeRole {
    load("display").unwrap_or_else(|| role(48.0, 56.0, "semibold"))
}

/// Pixel size for a named role. Fails closed in fidelity mode when the file
/// is still `unmeasured`.
pub fn px(role_name: &str) -> Result<f32, String> {
    if crate::fidelity_active() && crate::color::token_status("type.toml") != "measured" {
        return Err(format!("type.toml is unmeasured; refusing role {role_name}"));
    }
    let role = match role_name {
        "caption" => caption(),
        "body" => body(),
        "body-strong" => body_strong(),
        "subtitle" => subtitle(),
        "title" => title(),
        "display" => display(),
        other => return Err(format!("unknown type role {other}")),
    };
    Ok(role.size_px)
}

fn load(name: &str) -> Option<TypeRole> {
    let text = std::fs::read_to_string(crate::color::token_path("type.toml")).ok()?;
    let value: toml::Value = toml::from_str(&text).ok()?;
    let table = value.get(name)?.as_table()?;
    Some(TypeRole {
        size_px: table.get("size_px")?.as_float()? as f32,
        line_px: table.get("line_px")?.as_float()? as f32,
        weight: match table.get("weight").and_then(|v| v.as_str()).unwrap_or("regular") {
            "semibold" => "semibold",
            _ => "regular",
        },
        font: "ui",
    })
}
