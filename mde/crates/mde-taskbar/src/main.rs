//! Compatibility shim — taskbar lives in mde-shell.
use std::process::Command;

fn main() {
    eprintln!("mde-taskbar: taskbar is part of mde-shell; forwarding");
    let status = Command::new("mde-shell").status();
    match status {
        Ok(s) => std::process::exit(s.code().unwrap_or(0)),
        Err(e) => {
            eprintln!("mde-taskbar: failed to launch mde-shell: {e}");
            std::process::exit(1);
        }
    }
}
