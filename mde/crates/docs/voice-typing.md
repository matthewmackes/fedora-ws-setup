# Voice typing / dictation (W424)

Offline-first speech-to-text into the focused field (Super+H). Network STT is a fallback when configured.

## Session UX

1. **Super+H** — start listening (mic OSD pill).
2. **Super+H** again — stop, run STT, inject text, dismiss OSD.
3. Injection order: `wtype` → `ydotool type` → `wl-copy` + paste (`ctrl+v` via wtype/ydotool).

## Offline models (manual install)

MDE does **not** auto-download models. Place them under:

```text
~/.local/share/mde/stt/
```

### vosk (preferred offline)

```bash
mkdir -p ~/.local/share/mde/stt
# Example small English model:
curl -L -o /tmp/vosk-model.zip \
  https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip
unzip -d ~/.local/share/mde/stt /tmp/vosk-model.zip
# Expect: ~/.local/share/mde/stt/vosk-model-small-en-us-0.15/

# CLI (Fedora / pip):
#   sudo dnf install vosk   # if packaged
#   # or: pipx install vosk
```

MDE looks for `vosk` / `vosk-transcriber` on `PATH`, or a helper script:

```bash
~/.local/share/mde/stt/transcribe.sh <wav> <outfile>
```

### whisper.cpp

```bash
# Build whisper.cpp elsewhere, then:
mkdir -p ~/.local/share/mde/stt/whisper
# Place ggml model, e.g. ggml-base.en.bin
# Install `whisper-cli` or `main` from whisper.cpp on PATH as `whisper-cli`
```

MDE invokes:

```bash
whisper-cli -m ~/.local/share/mde/stt/whisper/ggml-base.en.bin -f <wav> -otxt -of <out-prefix>
```

## Network fallback

Set in `~/.config/mde/config.toml` extras (same as Start voice search):

```toml
[extras]
# JSON string keys in shell extras map:
# "start.voice_stt_url" = "https://your-stt.example/transcribe"
```

Or profile extras via Settings. Dictation POSTs the WAV like Start voice search when offline engines are missing.

## First-use empty state

If no model/CLI and no network URL: status shows install hint pointing at this doc. No fake transcription.
