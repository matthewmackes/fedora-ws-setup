//! 11×11 Gaussian SSIM and mean absolute error on RGBA images.

use image::RgbaImage;

const WINDOW: i32 = 11;
const SIGMA: f64 = 1.5;

#[derive(Debug, Clone)]
pub struct Score {
    pub ssim: f64,
    pub mae: f64,
}

pub fn score(actual: &RgbaImage, expected: &RgbaImage) -> Result<Score, String> {
    if actual.dimensions() != expected.dimensions() {
        return Err(format!(
            "size {:?} != {:?}",
            actual.dimensions(),
            expected.dimensions()
        ));
    }
    Ok(Score {
        ssim: ssim_luma(actual, expected),
        mae: mae(actual, expected),
    })
}

pub fn mae(actual: &RgbaImage, expected: &RgbaImage) -> f64 {
    let (w, h) = actual.dimensions();
    if w == 0 || h == 0 {
        return 0.0;
    }
    let mut acc = 0.0;
    let mut n = 0.0;
    for y in 0..h {
        for x in 0..w {
            let a = actual.get_pixel(x, y);
            let b = expected.get_pixel(x, y);
            for c in 0..3 {
                acc += (a[c] as f64 - b[c] as f64).abs() / 255.0;
                n += 1.0;
            }
        }
    }
    acc / n
}

fn luma(px: &image::Rgba<u8>) -> f64 {
    0.299 * px[0] as f64 + 0.587 * px[1] as f64 + 0.114 * px[2] as f64
}

fn kernel() -> Vec<f64> {
    let radius = WINDOW / 2;
    let mut k = Vec::with_capacity((WINDOW * WINDOW) as usize);
    let mut sum = 0.0;
    for y in -radius..=radius {
        for x in -radius..=radius {
            let g = (-((x * x + y * y) as f64) / (2.0 * SIGMA * SIGMA)).exp();
            k.push(g);
            sum += g;
        }
    }
    for v in &mut k {
        *v /= sum;
    }
    k
}

fn ssim_luma(actual: &RgbaImage, expected: &RgbaImage) -> f64 {
    let (w, h) = actual.dimensions();
    let radius = WINDOW / 2;
    if w < WINDOW as u32 || h < WINDOW as u32 {
        // Too small for the window. Fall back to a global luminance SSIM.
        return global_ssim(actual, expected);
    }
    let k = kernel();
    let c1 = (0.01 * 255.0_f64).powi(2);
    let c2 = (0.03 * 255.0_f64).powi(2);
    let mut acc = 0.0;
    let mut n = 0.0;
    for y in radius..h as i32 - radius {
        for x in radius..w as i32 - radius {
            let mut mu_a = 0.0;
            let mut mu_b = 0.0;
            let mut i = 0;
            for wy in -radius..=radius {
                for wx in -radius..=radius {
                    let weight = k[i];
                    i += 1;
                    mu_a += weight * luma(actual.get_pixel((x + wx) as u32, (y + wy) as u32));
                    mu_b += weight * luma(expected.get_pixel((x + wx) as u32, (y + wy) as u32));
                }
            }
            let mut var_a = 0.0;
            let mut var_b = 0.0;
            let mut cov = 0.0;
            i = 0;
            for wy in -radius..=radius {
                for wx in -radius..=radius {
                    let weight = k[i];
                    i += 1;
                    let da = luma(actual.get_pixel((x + wx) as u32, (y + wy) as u32)) - mu_a;
                    let db = luma(expected.get_pixel((x + wx) as u32, (y + wy) as u32)) - mu_b;
                    var_a += weight * da * da;
                    var_b += weight * db * db;
                    cov += weight * da * db;
                }
            }
            let num = (2.0 * mu_a * mu_b + c1) * (2.0 * cov + c2);
            let den = (mu_a * mu_a + mu_b * mu_b + c1) * (var_a + var_b + c2);
            acc += num / den;
            n += 1.0;
        }
    }
    if n == 0.0 {
        1.0
    } else {
        acc / n
    }
}

fn global_ssim(actual: &RgbaImage, expected: &RgbaImage) -> f64 {
    let (w, h) = actual.dimensions();
    let n = (w * h) as f64;
    if n == 0.0 {
        return 1.0;
    }
    let mut mu_a = 0.0;
    let mut mu_b = 0.0;
    for p in actual.pixels() {
        mu_a += luma(p);
    }
    for p in expected.pixels() {
        mu_b += luma(p);
    }
    mu_a /= n;
    mu_b /= n;
    let c1 = (0.01 * 255.0_f64).powi(2);
    (2.0 * mu_a * mu_b + c1) / (mu_a * mu_a + mu_b * mu_b + c1)
}

/// Absolute per-channel error, amplified ×4, written as an RGBA image.
pub fn diff_image(actual: &RgbaImage, expected: &RgbaImage) -> RgbaImage {
    let (w, h) = actual.dimensions();
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let a = actual.get_pixel(x, y);
            let b = expected.get_pixel(x, y);
            let mut px = image::Rgba([0, 0, 0, 255]);
            for c in 0..3 {
                let d = (a[c] as i16 - b[c] as i16).unsigned_abs();
                px[c] = (d.saturating_mul(4)).min(255) as u8;
            }
            out.put_pixel(x, y, px);
        }
    }
    out
}

/// Fill `rect` (x, y, w, h) with neutral gray on a copy. Used for `compare = "ignore"`.
pub fn mask_rect(image: &RgbaImage, rect: [u32; 4]) -> RgbaImage {
    let mut out = image.clone();
    let (iw, ih) = out.dimensions();
    let x0 = rect[0].min(iw);
    let y0 = rect[1].min(ih);
    let x1 = (rect[0] + rect[2]).min(iw);
    let y1 = (rect[1] + rect[3]).min(ih);
    for y in y0..y1 {
        for x in x0..x1 {
            out.put_pixel(x, y, image::Rgba([128, 128, 128, 255]));
        }
    }
    out
}
