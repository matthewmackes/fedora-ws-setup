//! `mde-session-ctl` — dedicated name for chrome recovery commands.
//! Re-execs `mde-session <cmd>` so there is a single implementation.
use std::os::unix::process::CommandExt;

fn main() {
    let mut args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if args.is_empty() {
        args.push("help".into());
    }
    let err = exec_mde_session(&args);
    eprintln!("mde-session-ctl: failed to exec mde-session: {err}");
    std::process::exit(1);
}

fn exec_mde_session(args: &[std::ffi::OsString]) -> std::io::Error {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("mde-session");
            if sibling.is_file() {
                return std::process::Command::new(&sibling).args(args).exec();
            }
        }
    }
    std::process::Command::new("mde-session").args(args).exec()
}
