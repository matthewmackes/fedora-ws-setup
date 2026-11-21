//! Session control plane: tab strip + output profile apply over a unix socket.

use crate::{outputs, tabs::{Tab, TabStrip}};
use anyhow::{Context, Result};
use mde_config::MdeConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tracing::{info, warn};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Ping,
    Tabs,
    TabAdd { id: u64, title: String, app_id: String },
    TabActivate { id: u64 },
    TabClose { id: u64 },
    ApplyProfile { name: String },
    Status,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Vec<TabDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TabDto {
    pub id: u64,
    pub title: String,
    pub app_id: String,
}

pub fn socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(runtime).join("mde-comp.sock")
}

pub async fn run_session() -> Result<()> {
    let cfg = MdeConfig::load().unwrap_or_default();
    info!(
        "mde-comp session control plane (layout={}, xwayland={}, fractional={})",
        cfg.active().compositor.layout,
        cfg.active().session.xwayland,
        cfg.active().displays.allow_fractional_scale
    );

    let path = socket_path();
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).with_context(|| format!("bind {}", path.display()))?;
    info!(path = %path.display(), "control socket listening");

    let mut strip = TabStrip::default();
    let mut cfg = cfg;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("session shutdown");
                break;
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _)) => {
                        if let Err(e) = handle_client(stream, &mut strip, &mut cfg).await {
                            warn!("client error: {e:#}");
                        }
                    }
                    Err(e) => warn!("accept failed: {e}"),
                }
            }
        }
    }

    let _ = std::fs::remove_file(&path);
    Ok(())
}

async fn handle_client(stream: UnixStream, strip: &mut TabStrip, cfg: &mut MdeConfig) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await? {
        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = Response {
                    ok: false,
                    error: Some(e.to_string()),
                    tabs: None,
                    active: None,
                    layout: None,
                    applied: None,
                };
                writer
                    .write_all(format!("{}\n", serde_json::to_string(&resp)?).as_bytes())
                    .await?;
                continue;
            }
        };
        let resp = dispatch(req, strip, cfg);
        writer
            .write_all(format!("{}\n", serde_json::to_string(&resp)?).as_bytes())
            .await?;
    }
    Ok(())
}

fn dispatch(req: Request, strip: &mut TabStrip, cfg: &mut MdeConfig) -> Response {
    match req {
        Request::Ping => Response {
            ok: true,
            error: None,
            tabs: None,
            active: None,
            layout: Some(cfg.active().compositor.layout.clone()),
            applied: None,
        },
        Request::Status | Request::Tabs => Response {
            ok: true,
            error: None,
            tabs: Some(
                strip
                    .tabs
                    .iter()
                    .map(|t| TabDto {
                        id: t.id,
                        title: t.title.clone(),
                        app_id: t.app_id.clone(),
                    })
                    .collect(),
            ),
            active: strip.active,
            layout: Some(cfg.active().compositor.layout.clone()),
            applied: None,
        },
        Request::TabAdd { id, title, app_id } => {
            strip.add(Tab { id, title, app_id });
            Response {
                ok: true,
                error: None,
                tabs: None,
                active: strip.active,
                layout: None,
                applied: None,
            }
        }
        Request::TabActivate { id } => Response {
            ok: strip.activate(id),
            error: None,
            tabs: None,
            active: strip.active,
            layout: None,
            applied: None,
        },
        Request::TabClose { id } => {
            strip.close(id);
            Response {
                ok: true,
                error: None,
                tabs: None,
                active: strip.active,
                layout: None,
                applied: None,
            }
        }
        Request::ApplyProfile { name } => {
            if let Ok(fresh) = MdeConfig::load() {
                *cfg = fresh;
            }
            match outputs::apply_profile(&cfg.active().displays, &name)
        {
            Ok(n) => Response {
                ok: true,
                error: None,
                tabs: None,
                active: None,
                layout: None,
                applied: Some(n),
            },
            Err(e) => Response {
                ok: false,
                error: Some(e.to_string()),
                tabs: None,
                active: None,
                layout: None,
                applied: None,
            },
        }
        }
    }
}

pub fn selftest() -> Result<()> {
    let mut strip = TabStrip::default();
    let mut cfg = MdeConfig::default();
    let r = dispatch(
        Request::TabAdd {
            id: 1,
            title: "Test".into(),
            app_id: "test".into(),
        },
        &mut strip,
        &mut cfg,
    );
    assert!(r.ok);
    let r = dispatch(Request::Ping, &mut strip, &mut cfg);
    assert_eq!(r.layout.as_deref(), Some("tabs"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_plane_selftest() {
        selftest().unwrap();
    }
}
