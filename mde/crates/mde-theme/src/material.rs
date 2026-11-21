//! Material kind. Widgets read this. They do not sample the backdrop.
//! Blur radius 0 means the effect spike has not shipped a compositor blur.

#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub kind: String,
    pub tint: String,
    pub tint_alpha: f32,
    pub blur: f32,
    pub noise: f32,
}

pub fn taskbar(dark: bool, high_contrast: bool) -> Material {
    load("taskbar", dark, high_contrast)
}
pub fn flyout(dark: bool, high_contrast: bool) -> Material {
    load("flyout", dark, high_contrast)
}
pub fn window(dark: bool, high_contrast: bool) -> Material {
    load("window", dark, high_contrast)
}

fn load(name: &str, dark: bool, high_contrast: bool) -> Material {
    let key = if high_contrast {
        if dark { "hc_black" } else { "hc_white" }
    } else if dark {
        "dark"
    } else {
        "light"
    };
    if let Some(material) = read(name, key) {
        return material;
    }
    if high_contrast {
        return Material {
            kind: "solid".into(),
            tint: if dark { "#000000".into() } else { "#ffffff".into() },
            tint_alpha: 1.0,
            blur: 0.0,
            noise: 0.0,
        };
    }
    Material {
        kind: "acrylic".into(),
        tint: if dark { "#2c2c2c".into() } else { "#f9f9f9".into() },
        tint_alpha: if name == "taskbar" {
            if dark { 0.70 } else { 0.78 }
        } else if dark {
            0.90
        } else {
            0.94
        },
        blur: 0.0,
        noise: 0.0,
    }
}

fn read(name: &str, key: &str) -> Option<Material> {
    let text = std::fs::read_to_string(crate::color::token_path("material.toml")).ok()?;
    let value: toml::Value = toml::from_str(&text).ok()?;
    let table = value.get(name)?.get(key)?.as_table()?;
    Some(Material {
        kind: table.get("kind")?.as_str()?.to_string(),
        tint: table.get("tint")?.as_str()?.to_string(),
        tint_alpha: table.get("tint_alpha")?.as_float()? as f32,
        blur: table.get("blur")?.as_float()? as f32,
        noise: table.get("noise")?.as_float()? as f32,
    })
}
