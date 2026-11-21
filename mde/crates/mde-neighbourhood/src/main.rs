//! Network Neighbourhood entry for the Rust platform.
use anyhow::Result;

fn main() -> Result<()> {
    // Prefer the existing GTK4 neighbourhood binary if installed; otherwise report.
    let candidates = [
        "network-neighbourhood",
        "mde-network-neighbourhood",
    ];
    for bin in candidates {
        if which(bin) {
            let err = std::process::Command::new(bin).args(std::env::args().skip(1)).status()?;
            std::process::exit(err.code().unwrap_or(1));
        }
    }
    println!(
        "{}: build mde-network-neighbourhood or install network-neighbourhood",
        mde_theme::about_line()
    );
    Ok(())
}

fn which(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p).any(|dir| dir.join(bin).is_file())
        })
        .unwrap_or(false)
}
