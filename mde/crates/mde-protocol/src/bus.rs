use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use tracing::{debug, warn};

use crate::{
    legacy::socket_path,
    messages::{ClientToComp, CompToShell, Envelope, Payload},
};

fn set_nonblocking(stream: &UnixStream) {
    let _ = stream.set_nonblocking(true);
}

fn set_blocking(stream: &UnixStream) {
    let _ = stream.set_nonblocking(false);
}

/// Compositor-side hub: accepts one shell client, queues inbound/outbound messages.
#[derive(Debug)]
pub struct CompBus {
    inbound: Mutex<VecDeque<ClientToComp>>,
    client: Mutex<Option<UnixStream>>,
    next_id: AtomicU64,
}

impl CompBus {
    /// Bind `$XDG_RUNTIME_DIR/mde.sock` and spawn an accept/reader thread.
    pub fn start() -> Arc<Self> {
        let bus = Arc::new(Self {
            inbound: Mutex::new(VecDeque::new()),
            client: Mutex::new(None),
            next_id: AtomicU64::new(1),
        });
        let path = socket_path();
        let _ = std::fs::remove_file(&path);
        match UnixListener::bind(&path) {
            Ok(listener) => {
                let bus_bg = Arc::clone(&bus);
                thread::Builder::new()
                    .name("mde-protocol-comp".into())
                    .spawn(move || CompBus::accept_loop(bus_bg, listener))
                    .ok();
                debug!(path = %path.display(), "mde-protocol listening");
            }
            Err(e) => warn!(?e, path = %path.display(), "mde-protocol bind failed"),
        }
        bus
    }

    fn accept_loop(bus: Arc<Self>, listener: UnixListener) {
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    set_blocking(&stream);
                    let reader = match stream.try_clone() {
                        Ok(s) => s,
                        Err(e) => {
                            warn!(?e, "mde-protocol stream clone failed");
                            continue;
                        }
                    };
                    {
                        let mut slot = bus.client.lock().unwrap_or_else(|e| e.into_inner());
                        *slot = Some(stream);
                    }
                    CompBus::read_loop(&bus, reader);
                    let mut slot = bus.client.lock().unwrap_or_else(|e| e.into_inner());
                    *slot = None;
                }
                Err(e) => {
                    warn!(?e, "mde-protocol accept error");
                    thread::sleep(Duration::from_millis(200));
                }
            }
        }
    }

    fn read_loop(bus: &CompBus, stream: UnixStream) {
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if let Ok(env) = Envelope::decode_line(&line) {
                        if let Payload::C2S(msg) = env.msg {
                            if let Ok(mut q) = bus.inbound.lock() {
                                q.push_back(msg);
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
    }

    pub fn drain(&self) -> Vec<ClientToComp> {
        let Ok(mut q) = self.inbound.lock() else {
            return Vec::new();
        };
        q.drain(..).collect()
    }

    pub fn push(&self, msg: CompToShell) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let Ok(line) = Envelope::s2c(id, msg).encode_line() else {
            return;
        };
        let mut slot = self.client.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(stream) = slot.as_mut() {
            if stream.write_all(line.as_bytes()).is_err() {
                *slot = None;
            } else {
                let _ = stream.flush();
            }
        }
    }

    /// Push typed ShellCmd; write legacy file only when no shell client is connected
    /// (avoids double-toggle when both transports are live).
    pub fn shell_cmd(&self, cmd: crate::ShellCmd, legacy: bool) {
        let has_client = self
            .client
            .lock()
            .map(|c| c.is_some())
            .unwrap_or(false);
        if legacy && !has_client {
            crate::legacy::write_shell_cmd_legacy(&cmd);
        }
        self.push(CompToShell::ShellCmd(cmd));
    }

    pub fn has_client(&self) -> bool {
        self.client
            .lock()
            .map(|c| c.is_some())
            .unwrap_or(false)
    }
}

/// Shell-side client: connect to compositor hub, send/receive envelopes.
#[derive(Debug, Default)]
pub struct ShellClient {
    stream: Mutex<Option<UnixStream>>,
    pending: Mutex<VecDeque<CompToShell>>,
    next_id: AtomicU64,
    reader_started: Mutex<bool>,
}

impl ShellClient {
    pub fn connect() -> Arc<Self> {
        let client = Arc::new(Self::default());
        client.ensure_connected();
        client
    }

    fn ensure_connected(&self) {
        let mut slot = self.stream.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_some() {
            return;
        }
        let path = socket_path();
        match UnixStream::connect(&path) {
            Ok(stream) => {
                set_nonblocking(&stream);
                *slot = Some(stream);
                drop(slot);
                let mut started = self.reader_started.lock().unwrap_or_else(|e| e.into_inner());
                if !*started {
                    *started = true;
                    // Reader is polled via try_recv (nonblocking) on the shell tick —
                    // no background thread required for the client.
                }
            }
            Err(_) => {
                *slot = None;
            }
        }
    }

    pub fn send(&self, msg: ClientToComp) {
        self.ensure_connected();
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let Ok(line) = Envelope::c2s(id, msg).encode_line() else {
            return;
        };
        let mut slot = self.stream.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(stream) = slot.as_mut() {
            // Briefly block for the write.
            set_blocking(stream);
            if stream.write_all(line.as_bytes()).is_err() || stream.flush().is_err() {
                *slot = None;
            } else {
                set_nonblocking(stream);
            }
        }
    }

    /// Non-blocking drain of compositor → shell messages.
    pub fn try_recv(&self) -> Vec<CompToShell> {
        self.ensure_connected();
        let mut out = Vec::new();
        {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            out.extend(pending.drain(..));
        }
        let mut slot = self.stream.lock().unwrap_or_else(|e| e.into_inner());
        let Some(stream) = slot.as_mut() else {
            return out;
        };
        set_nonblocking(stream);
        let mut buf = [0u8; 8192];
        loop {
            match std::io::Read::read(stream, &mut buf) {
                Ok(0) => {
                    *slot = None;
                    break;
                }
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&buf[..n]);
                    for line in chunk.lines() {
                        if line.trim().is_empty() {
                            continue;
                        }
                        if let Ok(env) = Envelope::decode_line(line) {
                            if let Payload::S2C(msg) = env.msg {
                                out.push(msg);
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    *slot = None;
                    break;
                }
            }
        }
        out
    }

    pub fn focus_window(&self, id: u64, legacy: bool) {
        if legacy {
            crate::legacy::write_focus_legacy(&crate::LegacyFocus::Id(id));
        }
        self.send(ClientToComp::FocusWindow { id });
    }

    pub fn focus_index(&self, index: usize, legacy: bool) {
        if legacy {
            crate::legacy::write_focus_legacy(&crate::LegacyFocus::Index(index));
        }
        self.send(ClientToComp::FocusIndex { index });
    }

    pub fn snap(&self, layout: crate::SnapLayout, legacy: bool) {
        if legacy {
            crate::legacy::write_snap_legacy(&layout);
        }
        self.send(ClientToComp::Snap { layout });
    }

    pub fn window_action(&self, action: crate::WinAction, legacy: bool) {
        if legacy {
            crate::legacy::write_window_action_legacy(&action);
        }
        self.send(ClientToComp::WindowAction { action });
    }

    pub fn workspace(&self, op: crate::WorkspaceOp, legacy: bool) {
        if legacy {
            crate::legacy::write_workspace_legacy(&op);
        }
        self.send(ClientToComp::Workspace { op });
    }

    pub fn show_desktop(&self, legacy: bool) {
        if legacy {
            crate::legacy::write_show_desktop_legacy();
        }
        self.send(ClientToComp::ShowDesktop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::ShellCmd;
    use std::time::Duration;

    #[test]
    fn comp_shell_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("XDG_RUNTIME_DIR", dir.path());
        let bus = CompBus::start();
        thread::sleep(Duration::from_millis(50));
        let client = ShellClient::connect();
        thread::sleep(Duration::from_millis(50));
        bus.push(CompToShell::ShellCmd(ShellCmd::ToggleStart));
        thread::sleep(Duration::from_millis(50));
        let msgs = client.try_recv();
        assert!(
            msgs.iter().any(|m| matches!(m, CompToShell::ShellCmd(ShellCmd::ToggleStart))),
            "expected ToggleStart, got {msgs:?}"
        );
        client.send(ClientToComp::ShowDesktop);
        thread::sleep(Duration::from_millis(50));
        let inbound = bus.drain();
        assert!(
            inbound.iter().any(|m| matches!(m, ClientToComp::ShowDesktop)),
            "expected ShowDesktop, got {inbound:?}"
        );
    }
}
