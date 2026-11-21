//! Measurement and screenshot diff for the Windows 11 fidelity program.
//! Reference pixels come from `mde/fidelity/specs`. This crate does not invent them.

pub mod golden;
pub mod measure;
pub mod spec;
pub mod spikes;
pub mod ssim;

#[cfg(test)]
use image::{Rgba, RgbaImage};
#[cfg(test)]
use std::fs;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden::SSIM_MIN;

    fn solid(w: u32, h: u32, px: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba(px))
    }

    #[test]
    fn identical_pair_ssim_is_one() {
        let a = solid(48, 48, [32, 64, 128, 255]);
        let b = a.clone();
        let score = ssim::score(&a, &b).unwrap();
        assert!((score.ssim - 1.0).abs() < 1e-6, "{}", score.ssim);
        assert!(score.mae < 1e-9);
    }

    #[test]
    fn ten_level_shift_fails_and_writes_diff() {
        let a = solid(32, 32, [100, 100, 100, 255]);
        let mut b = a.clone();
        for px in b.pixels_mut() {
            for c in 0..3 {
                px[c] = px[c].saturating_add(10);
            }
        }
        let dir = std::env::temp_dir().join(format!("mde-fidelity-diff-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let golden = dir.join("golden.png");
        a.save(&golden).unwrap();
        let err = golden::compare(&b, &golden, &dir.join("case")).unwrap_err();
        assert!(err.contains("regression failed"), "{err}");
        assert!(dir.join("case/diff.png").is_file());
        assert!(dir.join("case/actual.png").is_file());
        assert!(dir.join("case/score.json").is_file());
        let score = ssim::score(&a, &b).unwrap();
        assert!(score.mae > golden::MAE_MAX);
        let _ = SSIM_MIN;
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unmeasured_spec_blocks_reference_score() {
        let img = solid(8, 8, [0, 0, 0, 255]);
        let dir = std::env::temp_dir().join(format!("mde-fidelity-ref-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let err = golden::compare_reference(&img, &img, "unmeasured", 0.93, 0.012, &dir).unwrap_err();
        assert!(err.contains("unmeasured"), "{err}");
    }

    #[test]
    fn taskbar_spec_is_unmeasured() {
        let spec = spec::load_named("taskbar").expect("taskbar spec");
        assert_eq!(spec.status, "unmeasured");
        assert!(spec::require_measured(&spec).is_err());
    }
}
