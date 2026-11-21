//! Length-prefixed greetd IPC (JSON). `MDE_GREETER_JSONL=1` switches to newline JSON
//! for tests.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::json;
use std::env;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

/// Default session binary and Wayland env for `start_session`.
pub fn session_command() -> (String, Vec<String>) {
    let bin = env::var("MDE_SESSION_BIN").unwrap_or_else(|_| "mde-session".into());
    let env_vars = vec![
        "XDG_SESSION_TYPE=wayland".into(),
        "XDG_CURRENT_DESKTOP=MDE".into(),
        "XDG_SESSION_DESKTOP=mde".into(),
    ];
    (bin, env_vars)
}

/// Human login names from `/etc/passwd` (uid ≥ 1000, skip `nobody`).
pub fn list_human_users() -> Vec<String> {
    let Ok(text) = std::fs::read_to_string("/etc/passwd") else {
        return Vec::new();
    };
    let mut users = Vec::new();
    for line in text.lines() {
        let mut parts = line.split(':');
        let name = parts.next().unwrap_or("");
        let _passwd = parts.next();
        let uid: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        if uid < 1000 || name.is_empty() || name == "nobody" {
            continue;
        }
        users.push(name.to_string());
    }
    users.sort();
    users
}

/// Prefer `MDE_GREETER_USER`, else first human user, else empty (editable field).
pub fn default_username() -> String {
    if let Ok(u) = env::var("MDE_GREETER_USER") {
        if !u.is_empty() {
            return u;
        }
    }
    list_human_users().into_iter().next().unwrap_or_default()
}

/// create_session → auth prompts → start_session(mde-session).
pub fn authenticate(sock_path: &str, username: &str, password: &str) -> Result<()> {
    if username.is_empty() {
        bail!("username is required");
    }
    let mut stream = UnixStream::connect(sock_path)
        .with_context(|| format!("connect GREETD_SOCK={sock_path}"))?;
    write_msg(
        &mut stream,
        &json!({ "type": "create_session", "username": username }),
    )?;
    let mut reader = BufReader::new(stream.try_clone()?);
    loop {
        let resp = read_msg(&mut reader)?;
        match resp.msg_type.as_str() {
            "success" => break,
            "auth_message" => {
                let style = resp.auth_message_type.as_deref().unwrap_or("");
                let answer = if style == "secret" || style == "visible" {
                    password
                } else {
                    ""
                };
                write_msg(
                    &mut stream,
                    &json!({
                        "type": "post_auth_message_response",
                        "response": answer
                    }),
                )?;
            }
            "error" => {
                let _ = write_msg(&mut stream, &json!({ "type": "cancel_session" }));
                bail!(
                    "{}",
                    resp.description.unwrap_or_else(|| "authentication failed".into())
                );
            }
            other => bail!("unexpected greetd message: {other}"),
        }
    }

    let (session_bin, env_vars) = session_command();
    write_msg(
        &mut stream,
        &json!({
            "type": "start_session",
            "cmd": [session_bin],
            "env": env_vars
        }),
    )?;
    let resp = read_msg(&mut reader)?;
    if resp.msg_type != "success" {
        bail!(
            "start_session failed: {}",
            resp.description.unwrap_or(resp.msg_type)
        );
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct GreetdMsg {
    #[serde(rename = "type")]
    msg_type: String,
    #[serde(default)]
    auth_message_type: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

fn jsonl_mode() -> bool {
    env::var_os("MDE_GREETER_JSONL").is_some()
}

fn write_msg(stream: &mut UnixStream, value: &serde_json::Value) -> Result<()> {
    if jsonl_mode() {
        let mut bytes = serde_json::to_vec(value)?;
        bytes.push(b'\n');
        stream.write_all(&bytes)?;
    } else {
        let payload = serde_json::to_vec(value)?;
        let len = (payload.len() as u32).to_le_bytes();
        stream.write_all(&len)?;
        stream.write_all(&payload)?;
    }
    stream.flush()?;
    Ok(())
}

fn read_msg(reader: &mut BufReader<UnixStream>) -> Result<GreetdMsg> {
    if jsonl_mode() {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        return Ok(serde_json::from_str(line.trim())?);
    }
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(serde_json::from_slice(&buf)?)
}

/// True when a Wayland/X11 display is available for the iced greeter.
pub fn has_display() -> bool {
    env::var_os("WAYLAND_DISPLAY").is_some() || env::var_os("DISPLAY").is_some()
}

/// Headless/smoke: JSONL, explicit smoke, or no display.
pub fn prefer_headless() -> bool {
    env::var_os("MDE_GREETER_JSONL").is_some()
        || env::var_os("MDE_GREETER_SMOKE").is_some()
        || env::var_os("MDE_GREETER_UI").is_none() && !has_display()
}

/// Resolve greetd socket path if set and present.
pub fn sock_path() -> Option<String> {
    let path = env::var("GREETD_SOCK").ok()?;
    if Path::new(&path).exists() || jsonl_mode() {
        Some(path)
    } else {
        // Still return path so connect errors surface; socket may appear late.
        Some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_username_from_env() {
        env::set_var("MDE_GREETER_USER", "testuser");
        assert_eq!(default_username(), "testuser");
        env::remove_var("MDE_GREETER_USER");
    }
}
