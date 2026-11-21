//! `mde-fidelity` — measure references and score screenshots.
//!
//! Subcommands write nothing that looks like a Win11 measurement unless the
//! operator passes a capture path and a point or rectangle.

use image::RgbaImage;
use mde_fidelity::measure;
use mde_fidelity::spec;
use std::env;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "help".into());
    let result = match cmd.as_str() {
        "bar" => cmd_bar(&args_vec(args)),
        "color" => cmd_color(&args_vec(args)),
        "scale" => cmd_scale(&args_vec(args)),
        "ssim" => cmd_ssim(&args_vec(args)),
        "spec" => cmd_spec(&args_vec(args)),
        "spikes" => cmd_spikes(),
        "help" | "-h" | "--help" => {
            eprintln!(
                "usage: mde-fidelity <bar|color|scale|ssim|spec|spikes> ...\n\
                 bar   --image PATH\n\
                 color --image PATH --x N --y N\n\
                 scale --logical x,y,w,h --physical x,y,w,h --factor 1.5\n\
                 ssim  --actual PATH --golden PATH --out DIR\n\
                 spec  SURFACE\n\
                 spikes"
            );
            Ok(())
        }
        other => Err(format!("unknown command {other}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("mde-fidelity: {err}");
            ExitCode::from(1)
        }
    }
}

fn args_vec(args: impl Iterator<Item = String>) -> Vec<String> {
    args.collect()
}

fn flag<'a>(args: &'a [String], name: &str) -> Result<&'a str, String> {
    args.windows(2)
        .find(|pair| pair[0] == format!("--{name}"))
        .map(|pair| pair[1].as_str())
        .ok_or_else(|| format!("missing --{name}"))
}

fn load(path: &str) -> Result<RgbaImage, String> {
    image::open(Path::new(path))
        .map(|img| img.into_rgba8())
        .map_err(|e| format!("{path}: {e}"))
}

fn cmd_bar(args: &[String]) -> Result<(), String> {
    let img = load(flag(args, "image")?)?;
    let height = measure::bottom_band_height(&img);
    let (w, h) = img.dimensions();
    let sample = measure::median_sample(&img, w / 2, h.saturating_sub(height.max(1) / 2 + (h - height)))?;
    println!("height_px={height}");
    println!("hex={}", sample.hex);
    println!("alpha={}", sample.alpha);
    Ok(())
}

fn cmd_color(args: &[String]) -> Result<(), String> {
    let img = load(flag(args, "image")?)?;
    let x: u32 = flag(args, "x")?.parse().map_err(|_| "--x".to_string())?;
    let y: u32 = flag(args, "y")?.parse().map_err(|_| "--y".to_string())?;
    let sample = measure::median_sample(&img, x, y)?;
    println!("hex={}", sample.hex);
    println!("alpha={}", sample.alpha);
    Ok(())
}

fn cmd_scale(args: &[String]) -> Result<(), String> {
    let logical = parse_rect(flag(args, "logical")?)?;
    let physical = parse_rect(flag(args, "physical")?)?;
    let factor: f64 = flag(args, "factor")?.parse().map_err(|_| "--factor".to_string())?;
    let err = measure::scale_rect_error(logical, physical, factor);
    println!("logical_px_error={err:.3}");
    if err > 1.0 {
        return Err(format!("scale mismatch {err:.3} px > 1"));
    }
    Ok(())
}

fn parse_rect(text: &str) -> Result<[f64; 4], String> {
    let parts: Vec<_> = text.split(',').collect();
    if parts.len() != 4 {
        return Err("rect needs x,y,w,h".into());
    }
    let mut out = [0.0; 4];
    for (i, part) in parts.iter().enumerate() {
        out[i] = part.trim().parse().map_err(|_| "rect number".to_string())?;
    }
    Ok(out)
}

fn cmd_ssim(args: &[String]) -> Result<(), String> {
    let actual = load(flag(args, "actual")?)?;
    let golden = Path::new(flag(args, "golden")?);
    let out = Path::new(flag(args, "out")?);
    let cmp = mde_fidelity::golden::compare(&actual, golden, out)?;
    println!("ssim={:.6}", cmp.score.ssim);
    println!("mae={:.6}", cmp.score.mae);
    Ok(())
}

fn cmd_spec(args: &[String]) -> Result<(), String> {
    let name = args.first().ok_or("missing surface name")?;
    let spec = spec::load_named(name)?;
    println!("surface={}", spec.surface);
    println!("status={}", spec.status);
    println!("revision={}", spec.revision);
    println!("states={}", spec.states.len());
    if let Err(err) = spec::require_measured(&spec) {
        println!("measured=false");
        println!("block={err}");
    } else {
        println!("measured=true");
    }
    Ok(())
}

fn cmd_spikes() -> Result<(), String> {
    let (_img, blur) = mde_fidelity::spikes::blur_scene();
    println!(
        "blur edge_px={:.1} not_flat={} not_red={} cpu_ms={:.2} gpu_ms=none",
        blur.edge_px, blur.bar_not_flat, blur.bar_not_pure_red, blur.cpu_ms
    );
    let mica = mde_fidelity::spikes::mica_report();
    println!(
        "mica mae={:.5} unchanged={}",
        mica.mae, mica.unchanged_behind
    );
    let shadow = mde_fidelity::spikes::shadow_probe();
    println!(
        "shadow edge_err={:.2} corner={} side={}",
        shadow.max_edge_error_px, shadow.corner_alpha, shadow.side_alpha
    );
    let motion = mde_fidelity::spikes::motion_report(true);
    println!(
        "motion p50_ms={:.2} p99_ms={:.2} final_err={:.3}",
        motion.p50_ms, motion.p99_ms, motion.final_error_px
    );
    match mde_fidelity::spikes::text_report() {
        Ok(text) => println!(
            "text advance_delta={:.3} stem_mae={:.5}",
            text.advance_delta_px, text.stem_mae
        ),
        Err(err) => println!("text error={err}"),
    }
    Ok(())
}
