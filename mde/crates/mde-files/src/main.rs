//! mde-files — launch path for the file manager.
//!
//! The window chrome is not drawn here. `mde/fidelity/specs/files.toml` is
//! unmeasured, so this process opens the path with `xdg-open` and exits.
//! A panic still goes to the session log.

fn main() {
    mde_config::supervisor::install_panic_hook();
    let mut args = std::env::args().skip(1);
    if args.any(|a| a == "--selftest") {
        println!("{{\"ok\":true,\"bin\":\"mde-files\",\"ui\":\"blocked-unmeasured\"}}");
        return;
    }
    let path = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let status = std::process::Command::new("xdg-open").arg(&path).status();
    match status {
        Ok(code) if code.success() => {}
        Ok(code) => {
            eprintln!("mde-files: xdg-open {code}");
            std::process::exit(code.code().unwrap_or(1));
        }
        Err(err) => {
            eprintln!("mde-files: {err}");
            std::process::exit(1);
        }
    }
}
