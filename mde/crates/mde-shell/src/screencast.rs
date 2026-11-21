//! Portal ScreenCast helper for W449 Connect / Wireless display.

use std::sync::Mutex;

use ashpd::desktop::screencast::{
    CursorMode, Screencast, SelectSourcesOptions, SourceType,
};
use ashpd::desktop::PersistMode;

static STOP: Mutex<Option<std::sync::mpsc::Sender<()>>> = Mutex::new(None);

/// Blocking start used from iced `Task::perform` / spawn_blocking.
pub fn start_blocking(include_audio: bool) -> Result<String, String> {
    let _ = include_audio; // portal audio when backend supports it
    stop();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(async {
        let proxy = Screencast::new()
            .await
            .map_err(|e| format!("ScreenCast portal unavailable: {e}"))?;
        let session = proxy
            .create_session(Default::default())
            .await
            .map_err(|e| format!("create_session: {e}"))?;
        proxy
            .select_sources(
                &session,
                SelectSourcesOptions::default()
                    .set_cursor_mode(CursorMode::EMBEDDED)
                    .set_sources(SourceType::MONITOR)
                    .set_multiple(false)
                    .set_persist_mode(PersistMode::DoNot),
            )
            .await
            .map_err(|e| format!("select_sources: {e}"))?;
        let response = proxy
            .start(&session, None, Default::default())
            .await
            .map_err(|e| format!("start sharing: {e}"))?
            .response()
            .map_err(|e| format!("start response: {e}"))?;
        let label = response
            .streams()
            .first()
            .map(|s| format!("stream {}", s.pipe_wire_node_id()))
            .unwrap_or_else(|| "monitor".into());
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        std::thread::Builder::new()
            .name("mde-screencast".into())
            .spawn(move || {
                let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                rt.block_on(async move {
                    let _ = rx.recv();
                    let _ = session.close().await;
                    drop(proxy);
                });
            })
            .map_err(|e| e.to_string())?;
        if let Ok(mut g) = STOP.lock() {
            *g = Some(tx);
        }
        Ok(label)
    })
}

pub fn stop() {
    if let Ok(mut g) = STOP.lock() {
        if let Some(tx) = g.take() {
            let _ = tx.send(());
        }
    }
}

pub fn is_active() -> bool {
    STOP.lock().ok().is_some_and(|g| g.is_some())
}
