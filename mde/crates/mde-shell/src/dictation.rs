//! W424: voice typing — offline-first STT, wtype/ydotool then paste, mic OSD session.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

#[derive(Debug, Clone, Default)]
pub struct Session {
    pub active: bool,
    pub started: Option<Instant>,
    /// Background recorder / STT child still running after stop.
    pub pending: bool,
}

impl Session {
    pub fn toggle_start(&mut self) {
        self.active = true;
        self.pending = false;
        self.started = Some(Instant::now());
    }

    pub fn mark_pending(&mut self) {
        self.active = false;
        self.pending = true;
    }

    pub fn clear(&mut self) {
        self.active = false;
        self.pending = false;
        self.started = None;
    }
}

fn runtime_dir() -> PathBuf {
    PathBuf::from(std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into()))
}

fn model_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/share/mde/stt")
}

fn which(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Paths used by the background job.
pub fn wav_path() -> PathBuf {
    runtime_dir().join("mde-dictation.wav")
}

pub fn out_path() -> PathBuf {
    runtime_dir().join("mde-dictation.txt")
}

pub fn pid_path() -> PathBuf {
    runtime_dir().join("mde-dictation.pid")
}

/// Start arecord/parec into wav; returns status string.
pub fn start_listening() -> Result<(), String> {
    let wav = wav_path();
    let _ = std::fs::remove_file(&wav);
    let _ = std::fs::remove_file(out_path());
    let _ = std::fs::remove_file(pid_path());

    let wav_s = wav.display().to_string();
    let script = if which("arecord") {
        format!("arecord -q -f cd -t wav '{wav_s}' & echo $! > '{}'", pid_path().display())
    } else if which("parec") {
        format!(
            "parec --file-format=wav '{wav_s}' & echo $! > '{}'",
            pid_path().display()
        )
    } else {
        return Err("No recorder (arecord/parec)".into());
    };
    Command::new("bash")
        .args(["-lc", &script])
        .spawn()
        .map_err(|e| format!("dictation start: {e}"))?;
    Ok(())
}

/// Stop recorder, run STT offline → network, write `mde-dictation.txt`.
pub fn stop_and_transcribe(network_url: Option<&str>) -> Result<(), String> {
    // Kill recorder
    if let Ok(pid_txt) = std::fs::read_to_string(pid_path()) {
        let pid = pid_txt.trim();
        if !pid.is_empty() {
            let _ = Command::new("kill").args([pid]).status();
        }
    }
    let _ = std::fs::remove_file(pid_path());

    let wav = wav_path();
    let out = out_path();
    let out_s = out.display().to_string();
    let wav_s = wav.display().to_string();
    let model = model_dir();
    let model_s = model.display().to_string();
    let url = network_url.unwrap_or("").to_string();

    // A069: network STT first when configured; offline models are secondary.
    let script = format!(
        r#"set +e
sleep 0.3
if [ ! -s '{wav_s}' ]; then
  echo 'no audio captured' > '{out_s}'
  exit 0
fi
URL='{url}'
if [ -n "$URL" ]; then
  code=$(curl -sS -o '{out_s}' -w '%{{http_code}}' --max-time 30 -F "file=@{wav_s};filename=dictation.wav" -H 'Accept: text/plain' "$URL" || echo 000)
  if [ "$code" = "200" ] || [ "$code" = "201" ]; then
    # Strip optional JSON {{"text":"…"}} wrapper to plain first line.
    if command -v python3 >/dev/null 2>&1; then
      python3 -c 'import json;p="{out_s}";t=open(p).read().strip()
try:
 d=json.loads(t);t=str(d.get("text") or d.get("transcript") or t)
except Exception: pass
open(p,"w").write((t.splitlines() or [""])[0])' 2>/dev/null || true
    fi
    exit 0
  fi
  echo "STT HTTP $code — check start.voice_stt_url" > '{out_s}'
  exit 0
fi
# Offline helpers (optional; network path is the polished A069 track)
if [ -x '{model_s}/transcribe.sh' ]; then
  '{model_s}/transcribe.sh' '{wav_s}' '{out_s}' && exit 0
fi
if command -v vosk-transcriber >/dev/null 2>&1; then
  VMODEL=$(ls -d '{model_s}'/vosk-model* 2>/dev/null | head -1)
  if [ -n "$VMODEL" ]; then
    vosk-transcriber -m "$VMODEL" -i '{wav_s}' > '{out_s}' 2>/dev/null && exit 0
  fi
fi
echo 'Set Settings → Start → Network STT endpoint (start.voice_stt_url)' > '{out_s}'
"#
    );
    Command::new("bash")
        .args(["-lc", &script])
        .spawn()
        .map_err(|e| format!("dictation STT: {e}"))?;
    Ok(())
}

/// Inject transcribed text into the focused client.
pub fn inject_text(text: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return "Dictation · empty".into();
    }
    if which("wtype") {
        if Command::new("wtype")
            .arg(text)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return format!("Dictation · typed ({})", text.chars().take(32).collect::<String>());
        }
    }
    if which("ydotool") {
        if Command::new("ydotool")
            .args(["type", text])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return format!("Dictation · ydotool ({})", text.chars().take(32).collect::<String>());
        }
    }
    // Clipboard + paste
    let _ = Command::new("wl-copy").arg(text).status();
    if which("wtype") {
        let _ = Command::new("wtype")
            .args(["-M", "ctrl", "-k", "v", "-m", "ctrl"])
            .status();
        return "Dictation · pasted (wl-copy + ctrl+v)".into();
    }
    if which("ydotool") {
        let _ = Command::new("ydotool")
            .args(["key", "ctrl+v"])
            .status();
        return "Dictation · pasted (ydotool)".into();
    }
    "Dictation · copied to clipboard (install wtype/ydotool to type)".into()
}

pub fn read_result_if_ready() -> Option<String> {
    let path = out_path();
    if !path.is_file() {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    Some(text.lines().next().unwrap_or("").trim().to_string())
}

pub fn has_offline_model() -> bool {
    let dir = model_dir();
    if Path::new(&dir.join("transcribe.sh")).is_file() {
        return true;
    }
    if std::fs::read_dir(&dir)
        .ok()
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .any(|e| e.file_name().to_string_lossy().starts_with("vosk-model"))
        })
        .unwrap_or(false)
    {
        return true;
    }
    dir.join("whisper").is_dir()
        && std::fs::read_dir(dir.join("whisper"))
            .ok()
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .any(|e| e.file_name().to_string_lossy().contains("ggml"))
            })
            .unwrap_or(false)
}
