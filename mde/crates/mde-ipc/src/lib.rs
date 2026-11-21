//! Sway IPC client used by MDE shell components.
//! Replaces the Python `Sway` class in `ws-taskbar/common.py`.

use byteorder::{NativeEndian, ReadBytesExt, WriteBytesExt};
use serde_json::Value;
use std::env;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;

const MAGIC: &[u8; 6] = b"i3-ipc";

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("SWAYSOCK not set and sway --get-socketpath failed")]
    NoSocket,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("sway error: {0}")]
    Sway(String),
}

#[repr(u32)]
#[derive(Clone, Copy, Debug)]
pub enum MessageType {
    RunCommand = 0,
    GetWorkspaces = 1,
    Subscribe = 2,
    GetOutputs = 3,
    GetTree = 4,
    GetMarks = 5,
    GetBarConfig = 6,
    GetVersion = 7,
    GetBindingModes = 8,
    GetConfig = 9,
    SendTick = 10,
    GetInputs = 100,
}

pub struct Sway {
    path: PathBuf,
}

impl Sway {
    pub fn connect() -> Result<Self, IpcError> {
        let path = if let Ok(p) = env::var("SWAYSOCK") {
            PathBuf::from(p)
        } else {
            let out = Command::new("sway")
                .arg("--get-socketpath")
                .output()
                .map_err(|_| IpcError::NoSocket)?;
            if !out.status.success() {
                return Err(IpcError::NoSocket);
            }
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.is_empty() {
                return Err(IpcError::NoSocket);
            }
            PathBuf::from(s)
        };
        Ok(Self { path })
    }

    pub fn socket_path(&self) -> &std::path::Path {
        &self.path
    }

    fn roundtrip(&self, msg_type: MessageType, payload: &[u8]) -> Result<Value, IpcError> {
        let mut stream = UnixStream::connect(&self.path)?;
        stream.write_all(MAGIC)?;
        stream.write_u32::<NativeEndian>(payload.len() as u32)?;
        stream.write_u32::<NativeEndian>(msg_type as u32)?;
        stream.write_all(payload)?;
        stream.flush()?;

        let mut magic = [0u8; 6];
        stream.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(IpcError::Sway("bad IPC magic".into()));
        }
        let len = stream.read_u32::<NativeEndian>()? as usize;
        let _rtype = stream.read_u32::<NativeEndian>()?;
        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf)?;
        Ok(serde_json::from_slice(&buf)?)
    }

    pub fn run(&self, command: &str) -> Result<Value, IpcError> {
        self.roundtrip(MessageType::RunCommand, command.as_bytes())
    }

    pub fn get_tree(&self) -> Result<Value, IpcError> {
        self.roundtrip(MessageType::GetTree, b"")
    }

    pub fn get_workspaces(&self) -> Result<Value, IpcError> {
        self.roundtrip(MessageType::GetWorkspaces, b"")
    }

    pub fn get_outputs(&self) -> Result<Value, IpcError> {
        self.roundtrip(MessageType::GetOutputs, b"")
    }

    pub fn get_inputs(&self) -> Result<Value, IpcError> {
        self.roundtrip(MessageType::GetInputs, b"")
    }

    pub fn get_version(&self) -> Result<Value, IpcError> {
        self.roundtrip(MessageType::GetVersion, b"")
    }

    /// Fire-and-forget `swaymsg exec -- <cmd>` style launch.
    pub fn exec(cmd: &str) {
        let _ = Command::new("swaymsg")
            .args(["exec", "--", cmd])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_type_values_match_i3_ipc() {
        assert_eq!(MessageType::RunCommand as u32, 0);
        assert_eq!(MessageType::GetTree as u32, 4);
    }
}
