//! Compatibility shim — Settings is mde-control.
use std::env;
use std::os::unix::process::CommandExt;
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let err = Command::new("mde-control").args(&args).exec();
    eprintln!("mde-settings: exec mde-control failed: {err}");
    std::process::exit(1);
}
