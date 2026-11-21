//! Client for the mde-comp session control socket (`mde-comp ctl …`).

use crate::session::{socket_path, Request, Response};
use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

pub fn request(req: Request) -> Result<Response> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path)
        .with_context(|| format!("connect {} (is mde-comp --session running?)", path.display()))?;
    let line = serde_json::to_string(&req)?;
    stream.write_all(line.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut reader = BufReader::new(stream);
    let mut resp_line = String::new();
    reader.read_line(&mut resp_line)?;
    let resp: Response = serde_json::from_str(resp_line.trim())
        .with_context(|| format!("parse response: {resp_line}"))?;
    if !resp.ok {
        bail!(resp.error.unwrap_or_else(|| "request failed".into()));
    }
    Ok(resp)
}

pub fn run(args: &[String]) -> Result<()> {
    let req = match args.first().map(String::as_str) {
        None | Some("ping") | Some("status") => Request::Status,
        Some("tabs") => Request::Tabs,
        Some("outputs") => {
            // W449: live heads from compositor JSON mirror (`mde-outputs.json`).
            let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
            let path = std::path::Path::new(&runtime).join("mde-outputs.json");
            let body = std::fs::read_to_string(&path).unwrap_or_else(|_| {
                serde_json::json!({ "outputs": [] }).to_string()
            });
            println!("{body}");
            return Ok(());
        }
        Some("apply-profile") => {
            let name = args.get(1).context("usage: mde-comp ctl apply-profile <name>")?;
            Request::ApplyProfile {
                name: name.clone(),
            }
        }
        Some("tab-add") => {
            let id: u64 = args
                .get(1)
                .context("usage: mde-comp ctl tab-add <id> <title> <app_id>")?
                .parse()?;
            let title = args.get(2).cloned().unwrap_or_default();
            let app_id = args.get(3).cloned().unwrap_or_default();
            Request::TabAdd { id, title, app_id }
        }
        Some("tab-activate") => {
            let id: u64 = args
                .get(1)
                .context("usage: mde-comp ctl tab-activate <id>")?
                .parse()?;
            Request::TabActivate { id }
        }
        Some("tab-close") => {
            let id: u64 = args
                .get(1)
                .context("usage: mde-comp ctl tab-close <id>")?
                .parse()?;
            Request::TabClose { id }
        }
        Some(other) => bail!("unknown ctl command: {other}"),
    };
    let resp = request(req)?;
    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}
