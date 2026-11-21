//! On-screen keys. Characters go to the focused client through `wtype`.

use std::process::Command;

pub fn type_token(token: &str) -> Result<(), String> {
    let mut cmd = Command::new("wtype");
    if let Some(name) = token.strip_prefix("key:") {
        cmd.args(["-k", name]);
    } else {
        cmd.arg(token);
    }
    cmd.spawn().map_err(|e| format!("wtype: {e}"))?;
    Ok(())
}

pub fn letter_rows(shift: bool) -> [&'static [&'static str]; 3] {
    if shift {
        [
            &["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"],
            &["A", "S", "D", "F", "G", "H", "J", "K", "L"],
            &["Z", "X", "C", "V", "B", "N", "M"],
        ]
    } else {
        [
            &["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
            &["a", "s", "d", "f", "g", "h", "j", "k", "l"],
            &["z", "x", "c", "v", "b", "n", "m"],
        ]
    }
}

pub fn symbol_rows() -> [&'static [&'static str]; 3] {
    [
        &["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
        &["-", "/", ":", ";", "(", ")", "$", "&", "@"],
        &[".", ",", "?", "!", "'", "\""],
    ]
}
