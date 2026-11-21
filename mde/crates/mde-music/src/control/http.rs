//! LAN HTTP remote control API.

use crate::model::Track;
use crate::player::Player;
use anyhow::Result;
use parking_lot::RwLock;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

#[derive(Clone)]
pub struct ControlState {
    pub player: Player,
    pub now: Arc<RwLock<Option<Track>>>,
}

pub fn spawn(state: ControlState, port: u16) {
    thread::spawn(move || {
        let bind = format!("0.0.0.0:{port}");
        let listener = match TcpListener::bind(&bind) {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("control HTTP bind failed: {e}");
                return;
            }
        };
        tracing::info!("Neighbourhood Music control API on http://0.0.0.0:{port}");
        for stream in listener.incoming().flatten() {
            let st = state.clone();
            thread::spawn(move || {
                let _ = handle(stream, st);
            });
        }
    });
}

fn handle(mut stream: TcpStream, state: ControlState) -> Result<()> {
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf)?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req.lines();
    let first = lines.next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let path = parts.next().unwrap_or("/");

    let (code, body, ctype) = match (method, path) {
        ("GET", "/status") | ("GET", "/api/status") => {
            let track = state.now.read().clone();
            let json = serde_json::json!({
                "playing": state.player.is_playing(),
                "position": state.player.position_secs(),
                "duration": state.player.duration_secs(),
                "volume": state.player.volume(),
                "track": track,
                "queue_len": state.player.queue().len(),
            });
            (200, json.to_string(), "application/json")
        }
        ("POST", "/play") | ("POST", "/api/play") => {
            let _ = state.player.play();
            (200, "{\"ok\":true}".into(), "application/json")
        }
        ("POST", "/pause") | ("POST", "/api/pause") => {
            let _ = state.player.pause();
            (200, "{\"ok\":true}".into(), "application/json")
        }
        ("POST", "/toggle") | ("POST", "/api/toggle") => {
            let playing = state.player.toggle().unwrap_or(false);
            (200, format!("{{\"playing\":{playing}}}"), "application/json")
        }
        ("POST", "/next") | ("POST", "/api/next") => {
            let _ = state.player.advance();
            (200, "{\"ok\":true}".into(), "application/json")
        }
        ("POST", "/previous") | ("POST", "/api/previous") => {
            let _ = state.player.previous_track();
            (200, "{\"ok\":true}".into(), "application/json")
        }
        ("GET", "/") => (
            200,
            "<html><body><h1>Neighbourhood Music</h1><p>POST /play /pause /toggle /next /previous — GET /status</p></body></html>"
                .into(),
            "text/html",
        ),
        _ if path.starts_with("/seek/") => {
            if let Some(secs) = path.trim_start_matches("/seek/").parse::<u64>().ok() {
                let _ = state.player.seek_secs(secs);
            }
            (200, "{\"ok\":true}".into(), "application/json")
        }
        _ if path.starts_with("/volume/") => {
            if let Some(v) = path.trim_start_matches("/volume/").parse::<f64>().ok() {
                state.player.set_volume(v);
            }
            (200, "{\"ok\":true}".into(), "application/json")
        }
        _ => (404, "{\"error\":\"not found\"}".into(), "application/json"),
    };

    let resp = format!(
        "HTTP/1.1 {code} OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(resp.as_bytes())?;
    Ok(())
}
