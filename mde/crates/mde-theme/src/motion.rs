//! Motion tokens. Durations stay 0 until a capture fits a curve.
//! `theme.animations = false` forces every action to 0 ms.

pub struct Motion {
    pub duration_ms: u32,
    pub cubic_bezier: [f32; 4],
}

pub fn action(id: &str, animations: bool) -> Motion {
    if !animations {
        return Motion {
            duration_ms: 0,
            cubic_bezier: [0.0, 0.0, 1.0, 1.0],
        };
    }
    load(id).unwrap_or(Motion {
        duration_ms: 0,
        cubic_bezier: [0.0, 0.0, 1.0, 1.0],
    })
}

fn load(id: &str) -> Option<Motion> {
    let text = std::fs::read_to_string(crate::color::token_path("motion.toml")).ok()?;
    let value: toml::Value = toml::from_str(&text).ok()?;
    if crate::fidelity_active() {
        let status = value.get("status").and_then(|v| v.as_str()).unwrap_or("unmeasured");
        if status != "measured" {
            return Some(Motion {
                duration_ms: 0,
                cubic_bezier: [0.0, 0.0, 1.0, 1.0],
            });
        }
    }
    let table = value.get(id)?.as_table()?;
    let bez = table.get("cubic_bezier")?.as_array()?;
    if bez.len() != 4 {
        return None;
    }
    let mut cubic = [0.0; 4];
    for (i, v) in bez.iter().enumerate() {
        cubic[i] = v.as_float()? as f32;
    }
    Some(Motion {
        duration_ms: table.get("duration_ms")?.as_integer()? as u32,
        cubic_bezier: cubic,
    })
}
