//! Measurements taken from reference captures. No Win11 numbers are hardcoded.

use image::RgbaImage;

/// Median sRGB of a 5×5 neighbourhood. Alpha is reported only when the sample
/// sits on a flat `#ff00ff` sheet; otherwise alpha is `"unknown"`.
pub fn median_sample(image: &RgbaImage, x: u32, y: u32) -> Result<Sample, String> {
    let (w, h) = image.dimensions();
    if x >= w || y >= h {
        return Err("sample point outside image".into());
    }
    let mut rs = Vec::new();
    let mut gs = Vec::new();
    let mut bs = Vec::new();
    let mut alphas = Vec::new();
    for dy in -2i32..=2 {
        for dx in -2i32..=2 {
            let xx = x as i32 + dx;
            let yy = y as i32 + dy;
            if xx < 0 || yy < 0 || xx >= w as i32 || yy >= h as i32 {
                continue;
            }
            let p = image.get_pixel(xx as u32, yy as u32);
            rs.push(p[0]);
            gs.push(p[1]);
            bs.push(p[2]);
            alphas.push(p[3]);
        }
    }
    rs.sort_unstable();
    gs.sort_unstable();
    bs.sort_unstable();
    alphas.sort_unstable();
    let mid = rs.len() / 2;
    let hex = format!("#{:02x}{:02x}{:02x}", rs[mid], gs[mid], bs[mid]);
    let alpha = if alphas[mid] == 255 {
        "opaque".into()
    } else {
        "unknown".into()
    };
    Ok(Sample { hex, alpha })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    pub hex: String,
    pub alpha: String,
}

/// Height of a solid band painted along the bottom edge.
pub fn bottom_band_height(image: &RgbaImage) -> u32 {
    let (w, h) = image.dimensions();
    if h == 0 || w == 0 {
        return 0;
    }
    let probe_x = w / 2;
    let bottom = *image.get_pixel(probe_x, h - 1);
    let mut height = 0;
    for y in (0..h).rev() {
        let px = image.get_pixel(probe_x, y);
        if px[0] == bottom[0] && px[1] == bottom[1] && px[2] == bottom[2] {
            height += 1;
        } else {
            break;
        }
    }
    height
}

/// Logical rectangles from a 1.0 capture and a scaled capture.
/// Returns the max absolute error in logical pixels after dividing by `scale`.
pub fn scale_rect_error(logical: [f64; 4], physical: [f64; 4], scale: f64) -> f64 {
    if scale <= 0.0 {
        return f64::MAX;
    }
    let mut max = 0.0;
    for i in 0..4 {
        let err = (logical[i] - physical[i] / scale).abs();
        if err > max {
            max = err;
        }
    }
    max
}

/// Fit a quarter-circle radius to a top-left rounded corner (opaque inside).
pub fn fit_quarter_radius(image: &RgbaImage) -> Option<f64> {
    let (w, h) = image.dimensions();
    if w < 4 || h < 4 {
        return None;
    }
    let mut best_r = None;
    let mut best_err = u32::MAX;
    let limit = w.min(h);
    for r in 2..limit {
        let mut err = 0u32;
        let rf = r as f64;
        for y in 0..h {
            for x in 0..w {
                let dx = x as f64 - rf;
                let dy = y as f64 - rf;
                let inside = x >= r || y >= r || dx * dx + dy * dy <= rf * rf;
                let opaque = image.get_pixel(x, y)[3] > 128;
                if inside != opaque {
                    err += 1;
                }
            }
        }
        if err < best_err {
            best_err = err;
            best_r = Some(r as f64);
        }
    }
    best_r.filter(|_| best_err < (w * h) / 20)
}

/// Rows of a crop that contain ink (alpha or non-background luma).
pub fn cap_height_px(image: &RgbaImage, background: u8) -> u32 {
    let (w, h) = image.dimensions();
    let mut top = None;
    let mut bottom = None;
    for y in 0..h {
        let mut ink = false;
        for x in 0..w {
            let p = image.get_pixel(x, y);
            if (p[0] as i16 - background as i16).abs() > 24 || p[3] < 250 {
                ink = true;
                break;
            }
        }
        if ink {
            if top.is_none() {
                top = Some(y);
            }
            bottom = Some(y);
        }
    }
    match (top, bottom) {
        (Some(t), Some(b)) => b - t + 1,
        _ => 0,
    }
}

/// First frame that moves more than 1 px from the start, through the first
/// frame within 1 px of the final position. Times are milliseconds.
pub fn motion_duration_ms(times_ms: &[u32], positions: &[f64]) -> Option<u32> {
    if times_ms.len() != positions.len() || times_ms.len() < 2 {
        return None;
    }
    let start = positions[0];
    let end = *positions.last().unwrap();
    let mut began = None;
    for (i, pos) in positions.iter().enumerate() {
        if (pos - start).abs() > 1.0 {
            began = Some(i);
            break;
        }
    }
    let began = began?;
    for (i, pos) in positions.iter().enumerate().skip(began) {
        if (pos - end).abs() <= 1.0 {
            return Some(times_ms[i].saturating_sub(times_ms[began]));
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct EasingFit {
    pub cubic: [f64; 4],
    pub max_error_ratio: f64,
}

/// Fit `cubic-bezier(x1, y1, x2, y2)` to normalized samples `(t, y)`.
/// Rejects the fit when the max error exceeds 8% of the travel.
pub fn fit_cubic_bezier(samples: &[(f64, f64)]) -> Result<EasingFit, String> {
    if samples.len() < 4 {
        return Err("need at least 4 samples".into());
    }
    let y0 = samples[0].1;
    let y1 = samples.last().unwrap().1;
    let travel = (y1 - y0).abs();
    if travel < 1e-6 {
        return Err("unfitted".into());
    }
    let mut best = None;
    let mut best_err = f64::MAX;
    let steps = [0.0, 0.2, 0.4, 0.6, 0.8, 1.0];
    for &x1 in &steps {
        for &y1c in &steps {
            for &x2 in &steps {
                for &y2c in &steps {
                    let mut max_e = 0.0;
                    for &(t, y) in samples {
                        let pred = bezier_y(t.clamp(0.0, 1.0), x1, y1c, x2, y2c);
                        let e = (pred - ((y - y0) / (y1 - y0))).abs();
                        if e > max_e {
                            max_e = e;
                        }
                    }
                    if max_e < best_err {
                        best_err = max_e;
                        best = Some([x1, y1c, x2, y2c]);
                    }
                }
            }
        }
    }
    let cubic = best.ok_or("unfitted")?;
    if best_err > 0.08 {
        return Err("unfitted".into());
    }
    Ok(EasingFit {
        cubic,
        max_error_ratio: best_err,
    })
}

fn bezier_y(t: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    // Solve Bx(u) = t by bisection, then return By(u).
    let mut lo = 0.0;
    let mut hi = 1.0;
    for _ in 0..24 {
        let mid = (lo + hi) * 0.5;
        if bezier_comp(mid, x1, x2) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bezier_comp((lo + hi) * 0.5, y1, y2)
}

fn bezier_comp(u: f64, c1: f64, c2: f64) -> f64 {
    let o = 1.0 - u;
    3.0 * o * o * u * c1 + 3.0 * o * u * u * c2 + u * u * u
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn synthetic_bar_height_and_color() {
        let mut img = RgbaImage::from_pixel(64, 96, Rgba([255, 255, 255, 255]));
        for y in 48..96 {
            for x in 0..64 {
                img.put_pixel(x, y, Rgba([0x33, 0x66, 0x99, 255]));
            }
        }
        assert_eq!(bottom_band_height(&img), 48);
        let sample = median_sample(&img, 32, 72).unwrap();
        assert_eq!(sample.hex, "#336699");
    }

    #[test]
    fn one_point_five_scale_agrees_within_one_px() {
        let logical = [0.0, 1032.0, 1920.0, 48.0];
        let physical = [0.0, 1548.0, 2880.0, 72.0];
        assert!(scale_rect_error(logical, physical, 1.5) <= 1.0);
        let bad = [0.0, 1600.0, 2880.0, 72.0];
        assert!(scale_rect_error(logical, bad, 1.5) > 1.0);
    }

    #[test]
    fn quarter_circle_radius_recovers_eight() {
        let mut img = RgbaImage::from_pixel(32, 32, Rgba([0, 0, 0, 0]));
        let r = 8.0;
        for y in 0..32 {
            for x in 0..32 {
                let dx = x as f64 - r;
                let dy = y as f64 - r;
                let inside = x as f64 >= r || y as f64 >= r || dx * dx + dy * dy <= r * r;
                if inside {
                    img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
                }
            }
        }
        let fitted = fit_quarter_radius(&img).expect("radius");
        assert!((fitted - 8.0).abs() < 2.0, "fitted {fitted}");
    }

    #[test]
    fn cap_height_counts_ink_rows() {
        let mut img = RgbaImage::from_pixel(8, 16, Rgba([255, 255, 255, 255]));
        for y in 4..12 {
            img.put_pixel(2, y, Rgba([0, 0, 0, 255]));
        }
        assert_eq!(cap_height_px(&img, 255), 8);
    }

    #[test]
    fn duration_and_easing_on_a_known_curve() {
        let mut times = Vec::new();
        let mut pos = Vec::new();
        let mut samples = Vec::new();
        for i in 0..11 {
            let t = i as f64 / 10.0;
            times.push((t * 200.0) as u32);
            let y = bezier_y(t, 0.2, 0.0, 0.0, 1.0);
            pos.push(y * 100.0);
            samples.push((t, y * 100.0));
        }
        let ms = motion_duration_ms(&times, &pos).unwrap();
        assert!(ms > 0 && ms <= 200, "{ms}");
        let fit = fit_cubic_bezier(&samples).expect("fit");
        assert!(fit.max_error_ratio <= 0.08);
    }
}
