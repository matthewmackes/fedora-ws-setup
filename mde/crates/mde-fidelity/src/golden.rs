//! Golden-file compare. `MDE_UPDATE_GOLDENS=1` is the only writer.
//! CI must not set that variable. A missing golden is a failure, not a pass.

use crate::ssim::{self, Score};
use image::RgbaImage;
use std::fs;
use std::path::Path;

pub const SSIM_MIN: f64 = 0.985;
pub const MAE_MAX: f64 = 1.5 / 255.0;

#[derive(Debug)]
pub struct Compare {
    pub score: Score,
    pub passed: bool,
}

pub fn update_requested() -> bool {
    matches!(
        std::env::var("MDE_UPDATE_GOLDENS").ok().as_deref(),
        Some("1") | Some("true")
    )
}

/// Compare `actual` to the golden at `golden`. On failure, write `actual.png`,
/// `diff.png`, and `score.json` under `out_dir`.
pub fn compare(
    actual: &RgbaImage,
    golden: &Path,
    out_dir: &Path,
) -> Result<Compare, String> {
    fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    if !golden.is_file() {
        if update_requested() {
            actual
                .save(golden)
                .map_err(|e| format!("write golden: {e}"))?;
            return Ok(Compare {
                score: Score { ssim: 1.0, mae: 0.0 },
                passed: true,
            });
        }
        return Err(format!(
            "missing golden {} (set MDE_UPDATE_GOLDENS=1 to create it)",
            golden.display()
        ));
    }
    let expected = image::open(golden)
        .map_err(|e| format!("open golden: {e}"))?
        .into_rgba8();
    let score = ssim::score(actual, &expected)?;
    let passed = score.ssim + f64::EPSILON >= SSIM_MIN && score.mae <= MAE_MAX;
    let json = format!(
        "{{\"ssim\":{:.6},\"mae\":{:.6},\"passed\":{}}}\n",
        score.ssim,
        score.mae,
        if passed { "true" } else { "false" }
    );
    fs::write(out_dir.join("score.json"), json).map_err(|e| e.to_string())?;
    if !passed {
        actual
            .save(out_dir.join("actual.png"))
            .map_err(|e| e.to_string())?;
        ssim::diff_image(actual, &expected)
            .save(out_dir.join("diff.png"))
            .map_err(|e| e.to_string())?;
        return Err(format!(
            "regression failed ssim={:.4} mae={:.4} (floors {SSIM_MIN} / {MAE_MAX})",
            score.ssim, score.mae
        ));
    }
    Ok(Compare { score, passed })
}

/// Reference compare. Does not update goldens. Returns an error when the spec
/// status is not `measured` so an unmeasured surface cannot pass.
pub fn compare_reference(
    actual: &RgbaImage,
    reference: &RgbaImage,
    spec_status: &str,
    ssim_min: f64,
    mae_max: f64,
    out_dir: &Path,
) -> Result<Compare, String> {
    if spec_status != "measured" {
        return Err(format!(
            "spec status is {spec_status}; reference score blocked until status = measured"
        ));
    }
    fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let score = ssim::score(actual, reference)?;
    let passed = score.ssim + f64::EPSILON >= ssim_min && score.mae <= mae_max;
    let json = format!(
        "{{\"ssim\":{:.6},\"mae\":{:.6},\"passed\":{},\"region\":\"reference\"}}\n",
        score.ssim,
        score.mae,
        if passed { "true" } else { "false" }
    );
    fs::write(out_dir.join("score.json"), json).map_err(|e| e.to_string())?;
    if !passed {
        actual
            .save(out_dir.join("actual.png"))
            .map_err(|e| e.to_string())?;
        ssim::diff_image(actual, reference)
            .save(out_dir.join("diff.png"))
            .map_err(|e| e.to_string())?;
        return Err(format!(
            "reference failed ssim={:.4} mae={:.4}",
            score.ssim, score.mae
        ));
    }
    Ok(Compare { score, passed })
}
