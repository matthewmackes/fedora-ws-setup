//! org.freedesktop.Notifications host (zbus). Forwards Notify into the shell UI.

use crate::notifications::{parse_actions, Notification};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{info, warn};
use zbus::object_server::SignalEmitter;
use zbus::{connection, interface};

#[derive(Clone, Default)]
pub struct NotifyHub {
    inner: Arc<Mutex<NotifyInner>>,
}

#[derive(Default)]
struct NotifyInner {
    next_id: u32,
    inbox: Vec<Notification>,
    dnd: bool,
    priority: bool,
    priority_apps: Vec<String>,
    muted_apps: Vec<String>,
    outbound: Vec<Outbound>,
    ui_closes: Vec<u32>,
}

enum Outbound {
    Invoke { id: u32, key: String },
    Closed { id: u32, reason: u32 },
}

impl NotifyHub {
    pub fn set_dnd(&self, dnd: bool) {
        if let Ok(mut g) = self.inner.lock() {
            g.dnd = dnd;
        }
    }

    pub fn set_priority(&self, priority: bool, apps: Vec<String>) {
        if let Ok(mut g) = self.inner.lock() {
            g.priority = priority;
            g.priority_apps = apps;
        }
    }

    pub fn set_muted_apps(&self, apps: Vec<String>) {
        if let Ok(mut g) = self.inner.lock() {
            g.muted_apps = apps;
        }
    }

    pub fn drain(&self) -> Vec<Notification> {
        self.inner
            .lock()
            .map(|mut g| std::mem::take(&mut g.inbox))
            .unwrap_or_default()
    }

    pub fn queue_invoke(&self, id: u32, key: String) {
        if let Ok(mut g) = self.inner.lock() {
            g.outbound.push(Outbound::Invoke { id, key });
        }
    }

    pub fn queue_closed(&self, id: u32, reason: u32) {
        if let Ok(mut g) = self.inner.lock() {
            g.outbound.push(Outbound::Closed { id, reason });
        }
    }

    /// Close requested by the app over D-Bus. The shell removes the toast; the signal is already sent.
    pub fn note_remote_close(&self, id: u32) {
        if let Ok(mut g) = self.inner.lock() {
            g.ui_closes.push(id);
        }
    }

    pub fn take_ui_closes(&self) -> Vec<u32> {
        self.inner
            .lock()
            .map(|mut g| std::mem::take(&mut g.ui_closes))
            .unwrap_or_default()
    }

    fn take_outbound(&self) -> Vec<Outbound> {
        self.inner
            .lock()
            .map(|mut g| std::mem::take(&mut g.outbound))
            .unwrap_or_default()
    }

    fn push(
        &self,
        app_name: &str,
        summary: &str,
        body: &str,
        urgency: u8,
        replaces_id: u32,
        actions: Vec<(String, String)>,
        progress: Option<f32>,
        desktop_entry: Option<String>,
    ) -> u32 {
        let mut g = self.inner.lock().expect("notify hub");
        if g.dnd && urgency < 2 {
            return 0;
        }
        if crate::notifications::is_muted_app(app_name, &g.muted_apps) {
            return 0;
        }
        if g.priority
            && urgency < 2
            && !crate::notifications::is_priority_app(app_name, &g.priority_apps)
        {
            return 0;
        }
        let id = if replaces_id != 0 {
            replaces_id
        } else {
            g.next_id = g.next_id.wrapping_add(1).max(1);
            g.next_id
        };
        g.inbox.retain(|n| n.id != id);
        g.inbox.push(Notification {
            id,
            app_name: app_name.into(),
            summary: summary.into(),
            body: body.into(),
            urgency,
            created_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            actions,
            progress,
            desktop_entry,
        });
        id
    }
}

struct NotificationsIface {
    hub: NotifyHub,
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationsIface {
    async fn notify(
        &self,
        app_name: &str,
        _replaces_id: u32,
        _app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<String>,
        hints: std::collections::HashMap<String, zbus::zvariant::Value<'_>>,
        _expire_timeout: i32,
    ) -> zbus::fdo::Result<u32> {
        let urgency = match hints.get("urgency") {
            Some(zbus::zvariant::Value::U8(u)) => *u,
            Some(zbus::zvariant::Value::I32(i)) => (*i).clamp(0, 2) as u8,
            _ => 1,
        };
        let progress = hint_progress(&hints);
        let desktop_entry = hint_desktop_entry(&hints);
        let mut parsed = parse_actions(&actions);
        // W395: inbound KDE Connect shares without actions get Open / Save / Dismiss.
        let app_l = app_name.to_lowercase();
        if parsed.is_empty()
            && (app_l.contains("kdeconnect") || app_l.contains("kde connect"))
        {
            parsed = crate::kdeconnect::inbound_toast_actions(body);
        }
        Ok(self.hub.push(
            app_name,
            summary,
            body,
            urgency,
            _replaces_id,
            parsed,
            progress,
            desktop_entry,
        ))
    }

    async fn close_notification(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        id: u32,
    ) -> zbus::fdo::Result<()> {
        self.hub.note_remote_close(id);
        let _ = Self::notification_closed(&emitter, id, 3).await;
        Ok(())
    }

    #[zbus(signal)]
    async fn action_invoked(emitter: &SignalEmitter<'_>, id: u32, action_key: &str)
        -> zbus::Result<()>;

    #[zbus(signal)]
    async fn notification_closed(emitter: &SignalEmitter<'_>, id: u32, reason: u32)
        -> zbus::Result<()>;

    async fn get_capabilities(&self) -> zbus::fdo::Result<Vec<String>> {
        Ok(vec![
            "body".into(),
            "actions".into(),
            "persistence".into(),
            "value".into(),
        ])
    }

    async fn get_server_information(
        &self,
    ) -> zbus::fdo::Result<(String, String, String, String)> {
        Ok((
            "mde-shell".into(),
            "Mackes Desktop Environment".into(),
            mde_theme::VERSION_FULL.into(),
            "1.2".into(),
        ))
    }
}


/// A004: Freedesktop `desktop-entry` hint for taskbar badge targeting.
fn hint_desktop_entry(
    hints: &std::collections::HashMap<String, zbus::zvariant::Value<'_>>,
) -> Option<String> {
    for key in ["desktop-entry", "desktop_entry", "x-kde-appname"] {
        if let Some(zbus::zvariant::Value::Str(s)) = hints.get(key) {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t.trim_end_matches(".desktop").to_string());
            }
        }
    }
    None
}

/// W438: real progress from Notify hints (`value` / `percentage` / `percent` / `progress`).
fn hint_progress(
    hints: &std::collections::HashMap<String, zbus::zvariant::Value<'_>>,
) -> Option<f32> {
    for key in ["value", "percentage", "percent", "progress", "x-kde-progress"] {
        if let Some(v) = hints.get(key) {
            if let Some(p) = value_to_progress(v) {
                return Some(p);
            }
        }
    }
    None
}

fn value_to_progress(v: &zbus::zvariant::Value<'_>) -> Option<f32> {
    let n = match v {
        zbus::zvariant::Value::U8(u) => *u as f64,
        zbus::zvariant::Value::I16(i) => *i as f64,
        zbus::zvariant::Value::U16(u) => *u as f64,
        zbus::zvariant::Value::I32(i) => *i as f64,
        zbus::zvariant::Value::U32(u) => *u as f64,
        zbus::zvariant::Value::I64(i) => *i as f64,
        zbus::zvariant::Value::U64(u) => *u as f64,
        zbus::zvariant::Value::F64(f) => *f,
        zbus::zvariant::Value::Str(s) => {
            let t = s.trim().trim_end_matches('%');
            t.parse::<f64>().ok()?
        }
        _ => return None,
    };
    if (0.0..=1.0).contains(&n) {
        Some(n as f32)
    } else if (0.0..=100.0).contains(&n) {
        Some((n as f32) / 100.0)
    } else {
        None
    }
}


/// Spawn the session-bus Notifications service. Best-effort; UI still works if name taken.
pub async fn serve(hub: NotifyHub) {
    let pump = hub.clone();
    let iface = NotificationsIface { hub };
    let built = connection::Builder::session()
        .and_then(|b| b.name("org.freedesktop.Notifications"))
        .and_then(|b| b.serve_at("/org/freedesktop/Notifications", iface));
    match built {
        Ok(builder) => match builder.build().await {
            Ok(conn) => {
                info!("claimed org.freedesktop.Notifications");
                if let Ok(emitter) = SignalEmitter::new(&conn, "/org/freedesktop/Notifications") {
                    tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(Duration::from_millis(200)).await;
                            for event in pump.take_outbound() {
                                let sent = match &event {
                                    Outbound::Invoke { id, key } => {
                                        NotificationsIface::action_invoked(&emitter, *id, key).await
                                    }
                                    Outbound::Closed { id, reason } => {
                                        NotificationsIface::notification_closed(&emitter, *id, *reason)
                                            .await
                                    }
                                };
                                if let Err(e) = sent {
                                    warn!("notification signal failed: {e}");
                                }
                            }
                        }
                    });
                }
                std::future::pending::<()>().await;
                drop(conn);
            }
            Err(e) => warn!("Notifications bus connect failed: {e}"),
        },
        Err(e) => warn!("Notifications setup failed: {e}"),
    }
}
