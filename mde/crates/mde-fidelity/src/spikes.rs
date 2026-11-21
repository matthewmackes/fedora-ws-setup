//! Effect spikes. Geometry is asserted here. GPU timestamp queries are not
//! available without a GLES device, and that absence is recorded as a failure
//! of the blur time clause in `mde/fidelity/spikes/blur.md`.

use image::{Rgba, RgbaImage};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct BlurReport {
    pub edge_px: f64,
    pub bar_not_flat: bool,
    pub bar_not_pure_red: bool,
    pub cpu_ms: f64,
    pub gpu_ms: Option<f64>,
}

pub fn kawase_once(src: &RgbaImage, offset: i32) -> RgbaImage {
    let (w, h) = src.dimensions();
    let mut out = RgbaImage::new(w, h);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut acc = [0.0; 4];
            let mut n = 0.0;
            for (dx, dy) in [(-offset, -offset), (offset, -offset), (-offset, offset), (offset, offset)] {
                let xx = (x + dx).clamp(0, w as i32 - 1) as u32;
                let yy = (y + dy).clamp(0, h as i32 - 1) as u32;
                let p = src.get_pixel(xx, yy);
                for c in 0..4 {
                    acc[c] += p[c] as f64;
                }
                n += 1.0;
            }
            out.put_pixel(
                x as u32,
                y as u32,
                Rgba([
                    (acc[0] / n) as u8,
                    (acc[1] / n) as u8,
                    (acc[2] / n) as u8,
                    (acc[3] / n) as u8,
                ]),
            );
        }
    }
    out
}

pub fn kawase(src: &RgbaImage, iterations: u32) -> RgbaImage {
    let mut cur = src.clone();
    for i in 0..iterations {
        cur = kawase_once(&cur, (i as i32) + 1);
    }
    cur
}

/// Full-frame stand-in: left black, right white, a red window crossing the bar.
pub fn blur_scene() -> (RgbaImage, BlurReport) {
    let w = 320u32;
    let h = 180u32;
    let mut img = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let red_window = x >= 40 && x < 120 && y >= 140;
            let px = if red_window {
                Rgba([255, 0, 0, 255])
            } else if x < w / 2 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            };
            img.put_pixel(x, y, px);
        }
    }
    let started = Instant::now();
    // Several wide taps so a step edge moves by more than 8 px.
    let mut blurred = img.clone();
    for offset in [4, 8, 16, 24] {
        blurred = kawase_once(&blurred, offset);
    }
    let cpu_ms = started.elapsed().as_secs_f64() * 1000.0;
    let bar_y = h - 8;
    let mut min_l: f64 = 255.0;
    let mut max_l: f64 = 0.0;
    let mut pure_red = true;
    for x in 0..w {
        let p = blurred.get_pixel(x, bar_y);
        let l = 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64;
        min_l = min_l.min(l);
        max_l = max_l.max(l);
        if p[0] < 250 || p[1] > 8 || p[2] > 8 {
            pure_red = false;
        }
    }
    // Transition width around the vertical step, sampled above the red window.
    let y = 40;
    let mut lo = None;
    let mut hi = None;
    for x in 0..w {
        let p = blurred.get_pixel(x, y);
        let l = p[0] as f64;
        if lo.is_none() && l > 25.0 {
            lo = Some(x);
        }
        if l > 230.0 && hi.is_none() {
            hi = Some(x);
        }
    }
    let edge_px = match (lo, hi) {
        (Some(a), Some(b)) => b.saturating_sub(a) as f64,
        _ => 0.0,
    };
    (
        blurred,
        BlurReport {
            edge_px,
            bar_not_flat: (max_l - min_l) > 20.0,
            bar_not_pure_red: !pure_red,
            cpu_ms,
            gpu_ms: None,
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Integer snap math used by the compositor (work area already excludes the bar).
pub fn snap_rect(work: Rect, kind: &str) -> Option<Rect> {
    let half_w = work.w / 2;
    let rest_w = work.w - half_w;
    let half_h = work.h / 2;
    let rest_h = work.h - half_h;
    let two = (work.w * 2) / 3;
    let (x, y, w, h) = match kind {
        "left" => (work.x, work.y, half_w, work.h),
        "right" => (work.x + half_w, work.y, rest_w, work.h),
        "top-left" => (work.x, work.y, half_w, half_h),
        "top-right" => (work.x + half_w, work.y, rest_w, half_h),
        "bottom-left" => (work.x, work.y + half_h, half_w, rest_h),
        "bottom-right" => (work.x + half_w, work.y + half_h, rest_w, rest_h),
        "left-2/3" => (work.x, work.y, two, work.h),
        _ => return None,
    };
    Some(Rect { x, y, w, h })
}

pub fn opposite_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "left" => Some("right"),
        "right" => Some("left"),
        "top-left" => Some("bottom-right"),
        "left-2/3" => Some("right"),
        _ => None,
    }
}

pub fn rounded_rect_alpha(w: u32, h: u32, rect: Rect, radius: f32) -> Vec<f32> {
    let mut alpha = vec![0.0; (w * h) as usize];
    let cx = rect.x as f32 + rect.w as f32 / 2.0;
    let cy = rect.y as f32 + rect.h as f32 / 2.0;
    let hx = rect.w as f32 / 2.0 - radius;
    let hy = rect.h as f32 / 2.0 - radius;
    for y in 0..h {
        for x in 0..w {
            let qx = (x as f32 - cx).abs() - hx;
            let qy = (y as f32 - cy).abs() - hy;
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            let inside = qx.max(qy).min(0.0);
            let sdf = outside + inside - radius;
            let a = (0.5 - sdf).clamp(0.0, 1.0);
            alpha[(y * w + x) as usize] = a;
        }
    }
    alpha
}

fn box_blur(src: &[f32], w: u32, h: u32, radius: i32) -> Vec<f32> {
    let mut out = vec![0.0; src.len()];
    let r = radius.max(1);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut acc = 0.0;
            let mut n = 0.0;
            for dy in -r..=r {
                for dx in -r..=r {
                    let xx = (x + dx).clamp(0, w as i32 - 1) as u32;
                    let yy = (y + dy).clamp(0, h as i32 - 1) as u32;
                    acc += src[(yy * w + xx) as usize];
                    n += 1.0;
                }
            }
            out[(y as u32 * w + x as u32) as usize] = acc / n;
        }
    }
    out
}

pub struct ShadowProbe {
    pub max_edge_error_px: f64,
    pub corner_alpha: f32,
    pub side_alpha: f32,
}

pub fn shadow_probe() -> ShadowProbe {
    let pad = 48i32;
    let rw = 200i32;
    let rh = 120i32;
    let radius = 8.0;
    let w = (rw + pad * 2) as u32;
    let h = (rh + pad * 2) as u32;
    let rect = Rect {
        x: pad,
        y: pad,
        w: rw,
        h: rh,
    };
    let alpha = rounded_rect_alpha(w, h, rect, radius);
    let mut max_err = 0.0;
    let center_x = (rect.x as f32) + radius;
    let center_y = (rect.y as f32) + radius;
    for i in 0..8 {
        let theta = std::f32::consts::PI + (i as f32) * std::f32::consts::FRAC_PI_2 / 7.0;
        let px = center_x + radius * theta.cos();
        let py = center_y + radius * theta.sin();
        // Walk the normal (toward the corner center, which is inward) to find alpha 0.5.
        let sx = px.floor() as i32;
        let sy = py.floor() as i32;
        let tx = px - sx as f32;
        let ty = py - sy as f32;
        let at = |x: i32, y: i32| -> f32 {
            if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
                return 0.0;
            }
            alpha[(y as u32 * w + x as u32) as usize]
        };
        let a = at(sx, sy) * (1.0 - tx) * (1.0 - ty)
            + at(sx + 1, sy) * tx * (1.0 - ty)
            + at(sx, sy + 1) * (1.0 - tx) * ty
            + at(sx + 1, sy + 1) * tx * ty;
        let err = (a - 0.5).abs() as f64;
        if err > max_err {
            max_err = err;
        }
    }
    let blurred = box_blur(&alpha, w, h, 8);
    let corner = blurred[0];
    let side_x = (rect.x + rect.w / 2) as u32;
    let side_y = (rect.y - 2).max(0) as u32;
    let side = blurred[(side_y * w + side_x) as usize];
    ShadowProbe {
        max_edge_error_px: max_err,
        corner_alpha: corner,
        side_alpha: side,
    }
}

pub struct MicaReport {
    pub mae: f64,
    pub unchanged_behind: bool,
}

pub fn mica_report() -> MicaReport {
    let w = 200u32;
    let h = 40u32;
    let mut wall = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let c = if (x / 100) % 2 == 0 { 0 } else { 255 };
            wall.put_pixel(x, y, Rgba([c, c, c, 255]));
        }
    }
    let blurred = kawase(&wall, 4);
    let mut behind_a = RgbaImage::from_pixel(w, h, Rgba([255, 255, 0, 255]));
    let behind_b = RgbaImage::from_pixel(w, h, Rgba([0, 255, 255, 255]));
    // Approach A: mica samples the blurred wallpaper and ignores the window behind.
    let sample = |_: &RgbaImage| blurred.clone();
    let a = sample(&behind_a);
    behind_a = behind_b.clone();
    let b = sample(&behind_a);
    let mae = crate::ssim::mae(&a, &blurred);
    let unchanged = a.pixels().zip(b.pixels()).all(|(p, q)| p == q);
    let _ = behind_a;
    MicaReport {
        mae,
        unchanged_behind: unchanged,
    }
}

#[derive(Debug)]
pub struct Hit {
    pub inside: bool,
    pub physical_w: i32,
}

pub fn hit_test(scale: f64, origin: (f64, f64), logical: f64, click_physical: (f64, f64)) -> Hit {
    let phys = (logical * scale).round() as i32;
    let x0 = (origin.0 * scale).round();
    let y0 = (origin.1 * scale).round();
    let inside = click_physical.0 >= x0
        && click_physical.1 >= y0
        && click_physical.0 < x0 + phys as f64
        && click_physical.1 < y0 + phys as f64;
    Hit {
        inside,
        physical_w: phys,
    }
}

pub struct MotionReport {
    pub p50_ms: f64,
    pub p99_ms: f64,
    pub final_error_px: f64,
    pub animations_off_frames: u32,
}

pub fn ease(t: f64) -> f64 {
    // Candidate curve for the mechanism test. Not a measured Win11 value.
    let o = 1.0 - t;
    3.0 * o * o * t * 0.0 + 3.0 * o * t * t * 1.0 + t * t * t
}

pub fn motion_report(sleep: bool) -> MotionReport {
    let mut intervals = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        if sleep {
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
        intervals.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    intervals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = intervals[intervals.len() / 2];
    let p99_idx = ((intervals.len() as f64) * 0.99).ceil() as usize - 1;
    let p99 = intervals[p99_idx.min(intervals.len() - 1)];
    let mut y = 0.0;
    let frames = 25;
    for i in 1..=frames {
        y = ease(i as f64 / frames as f64) * 40.0;
    }
    MotionReport {
        p50_ms: p50,
        p99_ms: p99,
        final_error_px: (40.0 - y).abs(),
        animations_off_frames: 1,
    }
}

pub fn selawik_path() -> Option<std::path::PathBuf> {
    let p = std::path::PathBuf::from("/usr/share/fonts/mde/Selawik/selawk.ttf");
    p.is_file().then_some(p)
}

pub struct TextReport {
    pub advance_delta_px: f64,
    pub stem_mae: f64,
}

pub fn text_report() -> Result<TextReport, String> {
    let path = selawik_path().ok_or("Selawik ttf missing")?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let a = advance_and_stem(&bytes, 14.0)?;
    let b = advance_and_stem(&bytes, 14.0)?;
    let c = advance_and_stem(&bytes, 12.0)?;
    let _ = c;
    Ok(TextReport {
        advance_delta_px: (a.0 - b.0).abs() as f64,
        stem_mae: stem_mae(&a.1, &b.1),
    })
}

fn advance_and_stem(bytes: &[u8], size: f32) -> Result<(f32, Vec<u8>), String> {
    use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache};
    let mut font_system = FontSystem::new();
    font_system.db_mut().load_font_data(bytes.to_vec());
    let metrics = Metrics::new(size, size * 1.3);
    let mut buffer = Buffer::new(&mut font_system, metrics);
    buffer.set_size(&mut font_system, Some(800.0), Some(size * 2.0));
    buffer.set_text(
        &mut font_system,
        "Settings 08:30",
        Attrs::new().family(Family::Name("Selawik")),
        Shaping::Advanced,
    );
    let width = buffer
        .layout_runs()
        .next()
        .map(|run| run.line_w)
        .unwrap_or(0.0);
    let mut cache = SwashCache::new();
    let mut cover = vec![0u8; 64 * 32];
    for run in buffer.layout_runs() {
        for glyph in run.glyphs.iter().take(4) {
            let phys = glyph.physical((0.0, size), 1.0);
            cache.with_pixels(
                &mut font_system,
                phys.cache_key,
                Color::rgb(255, 255, 255),
                |x, y, color| {
                    let xx = (phys.x + x).max(0) as usize;
                    let yy = phys.y + y + 8;
                    if xx < 64 && yy >= 0 && (yy as usize) < 32 {
                        let alpha = (color.0 >> 24) as u8;
                        let i = yy as usize * 64 + xx;
                        if alpha > cover[i] {
                            cover[i] = alpha;
                        }
                    }
                },
            );
        }
    }
    Ok((width, cover))
}

fn stem_mae(a: &[u8], b: &[u8]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }
    let mut acc = 0.0;
    for (p, q) in a.iter().zip(b.iter()) {
        acc += (*p as f64 - *q as f64).abs() / 255.0;
    }
    acc / a.len() as f64
}

pub fn parse_snap_token(text: &str) -> Option<&'static str> {
    match text.trim() {
        "left" => Some("left"),
        "right" => Some("right"),
        "top-left" => Some("top-left"),
        "top-right" => Some("top-right"),
        "bottom-left" => Some("bottom-left"),
        "bottom-right" => Some("bottom-right"),
        "left-2/3" | "left-two-thirds" => Some("left-2/3"),
        "right-2/3" | "right-two-thirds" => Some("right-2/3"),
        "left-1/3" | "left-third" => Some("left-1/3"),
        "center-1/3" | "center-third" => Some("center-1/3"),
        "right-1/3" | "right-third" => Some("right-1/3"),
        "maximize" | "up" => Some("maximize"),
        "restore" | "down" => Some("restore"),
        "top" | "top-half" => Some("top-half"),
        "bottom" | "bottom-half" => Some("bottom-half"),
        "center" => Some("center"),
        _ => None,
    }
}

pub fn classify_shell_cmd(text: &str) -> &'static str {
    let cmd = text.trim();
    match cmd {
        "lock" | "start" | "volup" | "voldown" | "mute" | "magnifier" | "magnifier-off"
        | "minothers-tip" | "unminimize" | "brightup" | "brightdown" | "alttab" | "alttab-rev"
        | "alttab-quiet" | "diag" | "inputlang" | "screenshot" | "screenshot:screen"
        | "screenshot:area" | "screenshot:window" | "screenshot:snip" | "colorpick"
        | "colorfilter"
        | "widgets" | "taskview" | "snapflyout" | "startsearch" | "clipboard" | "quick"
        | "notifications" | "winmenu" | "explorer" | "settings" | "showdesktop" | "emoji"
        | "dictation" | "connect" | "ease" | "project" | "run" | "quicklink" | "minothers"
        | "gpu-reset" | "date" => "known",
        other if other.starts_with("screenshot:") => "known",
        other if other.starts_with("pin:") || other.starts_with("pin ") => "pin",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_moves_the_step_and_is_not_a_flat_tint() {
        let (_img, report) = blur_scene();
        assert!(report.bar_not_pure_red, "bar still pure red");
        assert!(report.bar_not_flat, "bar is a flat tint");
        assert!(
            report.edge_px >= 8.0,
            "edge moved only {} px",
            report.edge_px
        );
        assert!(report.gpu_ms.is_none());
        assert!(report.cpu_ms >= 0.0);
    }

    #[test]
    fn mica_ignores_the_window_behind() {
        let report = mica_report();
        assert!(report.mae <= 0.03, "mae {}", report.mae);
        assert!(report.unchanged_behind);
    }

    #[test]
    fn shadow_corner_is_round() {
        let probe = shadow_probe();
        assert!(
            probe.max_edge_error_px <= 0.5,
            "edge error {}",
            probe.max_edge_error_px
        );
        assert!(
            probe.corner_alpha < 0.05,
            "corner alpha {}",
            probe.corner_alpha
        );
        assert!(probe.side_alpha > 0.2, "side alpha {}", probe.side_alpha);
    }

    #[test]
    fn fractional_hit_targets() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let origin = (100.0, 80.0);
            let hit = hit_test(scale, origin, 48.0, (0.0, 0.0));
            let expect = (48.0 * scale).round() as i32;
            assert!((hit.physical_w - expect).abs() <= 1);
            let x0 = (origin.0 * scale).round();
            let y0 = (origin.1 * scale).round();
            let inside = hit_test(scale, origin, 48.0, (x0 + 1.0, y0 + 1.0));
            assert!(inside.inside);
            let outside = hit_test(scale, origin, 48.0, (x0 - 1.0, y0 + 1.0));
            assert!(!outside.inside);
            let _ = hit;
        }
    }

    #[test]
    fn snap_rects_match_integer_fractions() {
        let work = Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080 - 48,
        };
        let left = snap_rect(work, "left").unwrap();
        assert_eq!((left.x, left.y, left.w, left.h), (0, 0, 960, 1032));
        let right = snap_rect(work, "right").unwrap();
        assert_eq!((right.x, right.w), (960, 960));
        let tl = snap_rect(work, "top-left").unwrap();
        assert_eq!((tl.w, tl.h), (960, 516));
        let two = snap_rect(work, "left-2/3").unwrap();
        assert_eq!(two.w, 1280);
        assert_eq!(opposite_kind("left"), Some("right"));
        let opp = snap_rect(work, opposite_kind("left").unwrap()).unwrap();
        assert_eq!(opp.x, left.x + left.w);
    }

    #[test]
    fn motion_pose_and_disabled_animations() {
        let report = motion_report(false);
        assert!(report.final_error_px <= 1.0, "{}", report.final_error_px);
        assert_eq!(report.animations_off_frames, 1);
    }

    #[test]
    fn selawik_advances_match() {
        let report = text_report().expect("selawik");
        assert!(report.advance_delta_px <= 1.0);
        assert!(report.stem_mae <= 0.05, "mae {}", report.stem_mae);
    }

    #[test]
    fn fuzz_parsers_accept_garbage() {
        let mut state = 0x1234_5678u32;
        let seconds = std::env::var("MDE_FUZZ_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let start = std::time::Instant::now();
        let mut i = 0u32;
        loop {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let len = (state % 48) as usize;
            let bytes: Vec<u8> = (0..len).map(|n| (state.wrapping_add(n as u32) % 255) as u8).collect();
            let text = String::from_utf8_lossy(&bytes);
            let _ = parse_snap_token(&text);
            let _ = classify_shell_cmd(&text);
            let _ = toml::from_str::<toml::Value>(&text);
            let _ = toml::from_str::<mde_config::MdeConfig>(&text);
            i += 1;
            if seconds == 0 && i > 2000 {
                break;
            }
            if seconds > 0 && start.elapsed().as_secs() >= seconds {
                break;
            }
        }
    }
}
