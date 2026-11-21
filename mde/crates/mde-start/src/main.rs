//! Compatibility shim — Start lives in mde-shell.
use std::process::Command;

fn main() {
    eprintln!("mde-start: Start is part of mde-shell; forwarding toggle");
    let status = Command::new("mde-shell").status();
    match status {
        Ok(s) => std::process::exit(s.code().unwrap_or(0)),
        Err(e) => {
            eprintln!("mde-start: failed to launch mde-shell: {e}");
            std::process::exit(1);
        }
    }
}
