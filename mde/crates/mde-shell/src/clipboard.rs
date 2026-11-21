//! Clipboard history (cliphist replacement).

use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct ClipEntry {
    pub id: u64,
    pub preview: String,
    pub full: String,
    pub pinned: bool,
    /// PNG or JPEG file. Empty for text clips.
    pub image_file: String,
    /// Empty means `text/plain`. `text/html` and `text/uri-list` are pasted back as that type.
    pub mime: String,
    /// When the entry was created (or loaded) for relative age labels (W68).
    pub created: Instant,
}

#[derive(Debug, Default)]
pub struct ClipboardHistory {
    pub enabled: bool,
    pub max_items: usize,
    pub items: Vec<ClipEntry>,
    next_id: u64,
    image_tick: u8,
    last_image_digest: u64,
    last_html_digest: u64,
    last_uri_digest: u64,
    cross_device: bool,
    sync_dir: Option<std::path::PathBuf>,
    seen_sync: std::collections::HashSet<u64>,
    /// Skip writing to the cloud sync folder (W163).
    pub sync_paused: bool,
}

impl ClipboardHistory {
    #[cfg(test)]
    pub fn new(enabled: bool, max_items: usize) -> Self {
        Self::with_sync(enabled, max_items, false, None)
    }

    pub fn with_sync(
        enabled: bool,
        max_items: usize,
        cross_device: bool,
        sync_dir: Option<std::path::PathBuf>,
    ) -> Self {
        let mut history = Self {
            enabled,
            max_items: max_items.max(1),
            items: Vec::new(),
            next_id: 0,
            image_tick: 0,
            last_image_digest: 0,
            last_html_digest: 0,
            last_uri_digest: 0,
            cross_device,
            sync_dir,
            seen_sync: std::collections::HashSet::new(),
            sync_paused: false,
        };
        history.load_pins();
        history
    }

    /// W283: update history depth and trim unpinned items.
    pub fn set_max_items(&mut self, max: usize) {
        self.max_items = max.max(1);
        self.retain_limit();
    }

    pub fn push(&mut self, text: &str) {
        self.push_text(text, true);
    }

    fn push_text(&mut self, text: &str, export: bool) {
        if !self.enabled {
            return;
        }
        let text = text.trim_end_matches('\n');
        if text.is_empty() {
            return;
        }
        let preview: String = text.chars().take(120).collect();
        let mut pinned = false;
        if let Some(index) = self.items.iter().position(|e| e.full == text && e.mime.is_empty()) {
            pinned = self.items[index].pinned;
            if index == 0 {
                return;
            }
            self.items.remove(index);
        }
        self.next_id += 1;
        self.items.insert(
            0,
            ClipEntry {
                id: self.next_id,
                preview,
                full: text.to_string(),
                pinned,
                image_file: String::new(),
                mime: String::new(),
                created: Instant::now(),
            },
        );
        self.retain_limit();
        if export {
            self.export_sync_text(text);
        }
    }

    /// Store a `text/html` offer. Plain text from the same copy stays a separate entry.
    pub fn push_html(&mut self, html: &str) -> bool {
        if !self.enabled {
            return false;
        }
        let html = html.trim_end_matches('\n');
        if html.is_empty() || html.len() > 256 * 1024 {
            return false;
        }
        let digest = fnv64(html.as_bytes());
        if digest == self.last_html_digest
            && self.items.first().is_some_and(|entry| entry.mime == "text/html")
        {
            return false;
        }
        let mut pinned = false;
        if let Some(index) = self
            .items
            .iter()
            .position(|entry| entry.mime == "text/html" && entry.full == html)
        {
            pinned = self.items[index].pinned;
            if index == 0 {
                self.last_html_digest = digest;
                return false;
            }
            self.items.remove(index);
        }
        self.last_html_digest = digest;
        self.next_id += 1;
        self.items.insert(
            0,
            ClipEntry {
                id: self.next_id,
                preview: format!("HTML · {}", html_preview(html)),
                full: html.to_string(),
                pinned,
                image_file: String::new(),
                mime: "text/html".into(),
                created: Instant::now(),
            },
        );
        self.retain_limit();
        true
    }

    /// Store a `text/uri-list` offer, such as copied files.
    pub fn push_uris(&mut self, text: &str) -> bool {
        if !self.enabled {
            return false;
        }
        let text = text.trim_end_matches('\n');
        if text.trim().is_empty() || text.len() > 64 * 1024 {
            return false;
        }
        let digest = fnv64(text.as_bytes());
        if digest == self.last_uri_digest
            && self
                .items
                .first()
                .is_some_and(|entry| entry.mime == "text/uri-list")
        {
            return false;
        }
        let mut pinned = false;
        if let Some(index) = self
            .items
            .iter()
            .position(|entry| entry.mime == "text/uri-list" && entry.full == text)
        {
            pinned = self.items[index].pinned;
            if index == 0 {
                self.last_uri_digest = digest;
                return false;
            }
            self.items.remove(index);
        }
        self.last_uri_digest = digest;
        self.next_id += 1;
        self.items.insert(
            0,
            ClipEntry {
                id: self.next_id,
                preview: uri_preview(text),
                full: text.to_string(),
                pinned,
                image_file: String::new(),
                mime: "text/uri-list".into(),
                created: Instant::now(),
            },
        );
        self.retain_limit();
        true
    }

    pub fn push_image(&mut self, bytes: &[u8], ext: &str) -> bool {
        if !self.enabled || bytes.len() < 32 || bytes.len() > 8 * 1024 * 1024 {
            return false;
        }
        let digest = fnv64(bytes);
        if digest == self.last_image_digest {
            return false;
        }
        if self.items.iter().any(|entry| entry.image_file.contains(&format!("{digest:x}"))) {
            self.last_image_digest = digest;
            return false;
        }
        let Some(dir) = clip_dir() else {
            return false;
        };
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("{digest:x}.{ext}"));
        if std::fs::write(&path, bytes).is_err() {
            return false;
        }
        self.last_image_digest = digest;
        self.next_id += 1;
        let kb = bytes.len() / 1024;
        self.items.insert(
            0,
            ClipEntry {
                id: self.next_id,
                preview: format!("Image · {kb} KB"),
                full: String::new(),
                pinned: false,
                image_file: path.display().to_string(),
                mime: String::new(),
                created: Instant::now(),
            },
        );
        self.retain_limit();
        true
    }

    pub fn toggle_pin(&mut self, id: u64) -> Option<bool> {
        const MAX_PINNED: usize = 10;
        let idx = self.items.iter().position(|e| e.id == id)?;
        if !self.items[idx].pinned {
            let pinned_n = self.items.iter().filter(|e| e.pinned).count();
            if pinned_n >= MAX_PINNED {
                return None;
            }
        }
        self.items[idx].pinned = !self.items[idx].pinned;
        let pinned = self.items[idx].pinned;
        self.retain_limit();
        self.save_pins();
        Some(pinned)
    }

    /// Reorder a pinned clip among pinned items only (W203). `delta` −1 = up, +1 = down.
    pub fn reorder_pinned(&mut self, id: u64, delta: i32) -> bool {
        if delta == 0 {
            return false;
        }
        let pinned_ids: Vec<u64> = self
            .items
            .iter()
            .filter(|e| e.pinned)
            .map(|e| e.id)
            .collect();
        let Some(pidx) = pinned_ids.iter().position(|x| *x == id) else {
            return false;
        };
        let dest = (pidx as i32 + delta).clamp(0, pinned_ids.len().saturating_sub(1) as i32) as usize;
        if dest == pidx {
            return false;
        }
        let a = pinned_ids[pidx];
        let b = pinned_ids[dest];
        let ia = self.items.iter().position(|e| e.id == a);
        let ib = self.items.iter().position(|e| e.id == b);
        let (Some(ia), Some(ib)) = (ia, ib) else {
            return false;
        };
        self.items.swap(ia, ib);
        self.save_pins();
        true
    }

    /// Drop unpinned items older than 24h (W143). Called on open/poll.
    pub fn drop_expired(&mut self) {
        const MAX_AGE: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);
        let removed: Vec<String> = self
            .items
            .iter()
            .filter(|e| !e.pinned && e.created.elapsed() > MAX_AGE)
            .map(|e| e.image_file.clone())
            .filter(|path| !path.is_empty())
            .collect();
        self.items
            .retain(|e| e.pinned || e.created.elapsed() <= MAX_AGE);
        for path in removed {
            if self.items.iter().any(|e| e.image_file == path) {
                continue;
            }
            let _ = std::fs::remove_file(path);
        }
    }

    /// Drop unpinned history. Pinned clips stay.
    pub fn clear_unpinned(&mut self) {
        let pinned_files: Vec<String> = self
            .items
            .iter()
            .filter(|e| e.pinned)
            .map(|e| e.image_file.clone())
            .collect();
        let removed: Vec<String> = self
            .items
            .iter()
            .filter(|e| !e.pinned)
            .map(|e| e.image_file.clone())
            .filter(|path| !path.is_empty())
            .collect();
        self.items.retain(|e| e.pinned);
        for path in removed {
            if pinned_files.iter().any(|kept| kept == &path) {
                continue;
            }
            let _ = std::fs::remove_file(path);
        }
    }

    fn retain_limit(&mut self) {
        const MAX_PINNED: usize = 10;
        let before: Vec<String> = self
            .items
            .iter()
            .filter(|entry| !entry.pinned && !entry.image_file.is_empty())
            .map(|entry| entry.image_file.clone())
            .collect();
        // Cap pinned clips (W68): keep the first MAX_PINNED pinned entries in list order.
        let mut pinned_kept = 0usize;
        for entry in &mut self.items {
            if entry.pinned {
                if pinned_kept < MAX_PINNED {
                    pinned_kept += 1;
                } else {
                    entry.pinned = false;
                }
            }
        }
        let mut kept = 0usize;
        let max = self.max_items;
        self.items.retain(|entry| {
            if entry.pinned {
                return true;
            }
            if kept < max {
                kept += 1;
                true
            } else {
                false
            }
        });
        for path in before {
            if self.items.iter().any(|entry| entry.image_file == path) {
                continue;
            }
            let _ = std::fs::remove_file(path);
        }
    }

    fn load_pins(&mut self) {
        let Some(path) = pin_path() else {
            return;
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        for full in text.split("\n\u{1e}\n") {
            let full = full.trim_end_matches('\n');
            if full.is_empty() {
                continue;
            }
            self.next_id += 1;
            let preview: String = full.chars().take(120).collect();
            let (full, image_file, preview, mime) = if let Some(path) = full.strip_prefix('\u{1f}') {
                (String::new(), path.to_string(), "Image · pinned".into(), String::new())
            } else if let Some(html) = full.strip_prefix('\u{1d}') {
                let preview = format!("HTML · {}", html_preview(html));
                (html.to_string(), String::new(), preview, "text/html".into())
            } else if let Some(uris) = full.strip_prefix('\u{1c}') {
                (uris.to_string(), String::new(), uri_preview(uris), "text/uri-list".into())
            } else {
                (full.to_string(), String::new(), preview, String::new())
            };
            self.items.push(ClipEntry {
                id: self.next_id,
                preview,
                full,
                pinned: true,
                image_file,
                mime,
                created: Instant::now(),
            });
        }
    }

    fn save_pins(&self) {
        let Some(path) = pin_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut body = String::new();
        for entry in self.items.iter().filter(|e| e.pinned) {
            if !entry.image_file.is_empty() {
                body.push('\u{1f}');
                body.push_str(&entry.image_file);
            } else if entry.mime == "text/html" {
                body.push('\u{1d}');
                body.push_str(&strip_marks(&entry.full));
            } else if entry.mime == "text/uri-list" {
                body.push('\u{1c}');
                body.push_str(&strip_marks(&entry.full));
            } else {
                body.push_str(&strip_marks(&entry.full));
            }
            body.push('\n');
            body.push('\u{1e}');
            body.push('\n');
        }
        let _ = std::fs::write(path, body);
    }

    pub fn paste_back(&self, id: u64) -> Result<(), String> {
        let Some(entry) = self.items.iter().find(|e| e.id == id) else {
            return Err("clipboard entry not found".into());
        };
        let (mime, bytes) = if !entry.image_file.is_empty() {
            let mime = if entry.image_file.ends_with(".jpg") {
                "image/jpeg"
            } else {
                "image/png"
            };
            let bytes = std::fs::read(&entry.image_file)
                .map_err(|e| format!("read image: {e}"))?;
            (Some(mime), bytes)
        } else if entry.mime == "text/html" {
            (Some("text/html"), entry.full.as_bytes().to_vec())
        } else if entry.mime == "text/uri-list" {
            (Some("text/uri-list"), entry.full.as_bytes().to_vec())
        } else {
            (None, entry.full.as_bytes().to_vec())
        };
        wl_copy_bytes(mime, &bytes)
    }

    /// Save an image clip as PNG under ~/Pictures/Clipboard (W243).
    pub fn save_image_to_pictures(&self, id: u64) -> Result<String, String> {
        let Some(entry) = self.items.iter().find(|e| e.id == id) else {
            return Err("clipboard entry not found".into());
        };
        if entry.image_file.is_empty() {
            return Err("Save image · no bytes available".into());
        }
        let bytes = std::fs::read(&entry.image_file)
            .map_err(|_| "Save image · no bytes available".to_string())?;
        if bytes.is_empty() {
            return Err("Save image · no bytes available".into());
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let dir = std::path::PathBuf::from(home).join("Pictures").join("Clipboard");
        std::fs::create_dir_all(&dir).map_err(|e| format!("Save image · {e}"))?;
        let dest = dir.join(format!("clip-{id}.png"));
        // Prefer re-encode to PNG when possible; fall back to raw copy.
        match image::load_from_memory(&bytes) {
            Ok(img) => img
                .save_with_format(&dest, image::ImageFormat::Png)
                .map_err(|e| format!("Save image · {e}"))?,
            Err(_) => {
                // Already PNG or exotic — write bytes as .png path.
                std::fs::write(&dest, &bytes).map_err(|e| format!("Save image · {e}"))?;
            }
        }
        Ok(dest.display().to_string())
    }

    /// Paste as plain text only (W123) — strips HTML tags / uses path list as text.
    pub fn paste_plain(&self, id: u64) -> Result<(), String> {
        let Some(entry) = self.items.iter().find(|e| e.id == id) else {
            return Err("clipboard entry not found".into());
        };
        if !entry.image_file.is_empty() {
            return Err("image clip has no plain text".into());
        }
        let plain = if entry.mime == "text/html" {
            strip_simple_html(&entry.full)
        } else {
            entry.full.clone()
        };
        wl_copy_bytes(None, plain.as_bytes())
    }

    /// W436: write clip to a temp file and offer `text/uri-list` (path ready to paste/open).
    pub fn paste_as_file(&self, id: u64) -> Result<String, String> {
        let Some(entry) = self.items.iter().find(|e| e.id == id) else {
            return Err("clipboard entry not found".into());
        };
        let dir = std::env::temp_dir().join("mde-clip-files");
        std::fs::create_dir_all(&dir).map_err(|e| format!("Paste as file · {e}"))?;
        let path = if !entry.image_file.is_empty() {
            let ext = if entry.image_file.ends_with(".jpg") {
                "jpg"
            } else {
                "png"
            };
            let dest = dir.join(format!("clip-{id}.{ext}"));
            std::fs::copy(&entry.image_file, &dest)
                .map_err(|e| format!("Paste as file · {e}"))?;
            dest
        } else if entry.mime == "text/uri-list" {
            // Already file paths — re-offer as-is.
            wl_copy_bytes(Some("text/uri-list"), entry.full.as_bytes())?;
            let first = entry
                .full
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with('#'))
                .unwrap_or("files");
            return Ok(format!("Paste as file · {first}"));
        } else {
            let body = if entry.mime == "text/html" {
                strip_simple_html(&entry.full)
            } else {
                entry.full.clone()
            };
            let dest = dir.join(format!("clip-{id}.txt"));
            std::fs::write(&dest, body.as_bytes())
                .map_err(|e| format!("Paste as file · {e}"))?;
            dest
        };
        let uri = format!("file://{}", path.display());
        wl_copy_bytes(Some("text/uri-list"), format!("{uri}\n").as_bytes())?;
        // Best-effort: also reveal in the file manager so the path is usable.
        let _ = Command::new("xdg-open").arg(&dir).spawn();
        Ok(format!("Paste as file · {}", path.display()))
    }

    fn resolve_sync_dir(&self) -> Option<std::path::PathBuf> {
        if !self.cross_device {
            return None;
        }
        if let Some(dir) = &self.sync_dir {
            if !dir.as_os_str().is_empty() {
                return Some(dir.clone());
            }
        }
        let home = std::env::var("HOME").ok()?;
        Some(std::path::PathBuf::from(home).join(".local/share/mde/clipboard-sync"))
    }

    /// Whether a cloud sync folder is configured (W163).
    pub fn sync_path_active(&self) -> bool {
        self.resolve_sync_dir().is_some()
    }

    /// Toggle pause; when pausing with a sync path, clear exported `.txt` clips (W163).
    pub fn set_sync_paused(&mut self, paused: bool) -> String {
        self.sync_paused = paused;
        if paused {
            if let Some(dir) = self.resolve_sync_dir() {
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        let path = entry.path();
                        if path.extension().and_then(|x| x.to_str()) == Some("txt") {
                            let _ = std::fs::remove_file(path);
                        }
                    }
                }
                "Clipboard sync paused · cleared cloud folder".into()
            } else {
                "Clipboard sync paused".into()
            }
        } else {
            "Clipboard sync resumed".into()
        }
    }

    fn export_sync_text(&self, text: &str) {
        if self.sync_paused {
            return;
        }
        let Some(dir) = self.resolve_sync_dir() else {
            return;
        };
        let _ = std::fs::create_dir_all(&dir);
        let digest = fnv64(text.as_bytes());
        let path = dir.join(format!("{digest:x}.txt"));
        if path.exists() {
            return;
        }
        let _ = std::fs::write(path, text);
        // Keep newest 100 sync files by mtime.
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        let mut files: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("txt"))
            .filter_map(|e| {
                let modified = e.metadata().ok()?.modified().ok()?;
                Some((modified, e.path()))
            })
            .collect();
        if files.len() <= 100 {
            return;
        }
        files.sort_by_key(|(modified, _)| *modified);
        let drop_count = files.len() - 100;
        for (_, path) in files.into_iter().take(drop_count) {
            let _ = std::fs::remove_file(path);
        }
    }

    /// Import text clips written by other devices into the shared sync folder.
    pub fn poll_sync(&mut self) {
        let Some(dir) = self.resolve_sync_dir() else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut imported = Vec::new();
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("txt") {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let Ok(modified) = meta.modified() else {
                continue;
            };
            let digest = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| u64::from_str_radix(s, 16).ok())
                .unwrap_or(0);
            if digest != 0 && self.seen_sync.contains(&digest) {
                continue;
            }
            imported.push((modified, path, digest));
        }
        imported.sort_by_key(|(modified, _, _)| *modified);
        for (_, path, digest) in imported.into_iter().rev().take(20) {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if digest != 0 {
                self.seen_sync.insert(digest);
            }
            // Avoid re-exporting remote clips back to the sync folder.
            self.push_text(&text, false);
        }
    }

    pub fn poll_primary(&mut self) {
        if !self.enabled {
            return;
        }
        self.drop_expired();
        self.poll_sync();
        let Ok(out) = Command::new("wl-paste").arg("-n").output() else {
            return;
        };
        if !out.status.success() {
            return;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        if !text.is_empty() {
            self.push(&text);
        }
        self.image_tick = self.image_tick.wrapping_add(1);
        if self.image_tick % 5 == 0 {
            self.poll_image();
            self.poll_html();
            self.poll_uris();
        }
    }

    fn poll_uris(&mut self) {
        let Ok(types) = Command::new("wl-paste").arg("-l").output() else {
            return;
        };
        if !types.status.success() {
            return;
        }
        let list = String::from_utf8_lossy(&types.stdout);
        if !list.lines().any(|line| line.trim() == "text/uri-list") {
            return;
        }
        let Ok(out) = Command::new("wl-paste").arg("-t").arg("text/uri-list").output() else {
            return;
        };
        if !out.status.success() || out.stdout.len() > 64 * 1024 {
            return;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let _ = self.push_uris(&text);
    }

    fn poll_html(&mut self) {
        let Ok(types) = Command::new("wl-paste").arg("-l").output() else {
            return;
        };
        if !types.status.success() {
            return;
        }
        let list = String::from_utf8_lossy(&types.stdout);
        if !list.lines().any(|line| line.trim() == "text/html") {
            return;
        }
        let Ok(out) = Command::new("wl-paste").arg("-t").arg("text/html").output() else {
            return;
        };
        if !out.status.success() || out.stdout.len() > 256 * 1024 {
            return;
        }
        let html = String::from_utf8_lossy(&out.stdout);
        let _ = self.push_html(&html);
    }

    fn poll_image(&mut self) {
        let Ok(types) = Command::new("wl-paste").arg("-l").output() else {
            return;
        };
        if !types.status.success() {
            return;
        }
        let list = String::from_utf8_lossy(&types.stdout);
        let mime = if list.lines().any(|line| line.trim() == "image/png") {
            ("image/png", "png")
        } else if list.lines().any(|line| line.trim() == "image/jpeg") {
            ("image/jpeg", "jpg")
        } else {
            return;
        };
        let Ok(out) = Command::new("wl-paste").arg("-t").arg(mime.0).output() else {
            return;
        };
        if out.status.success() {
            let _ = self.push_image(&out.stdout, mime.1);
        }
    }
}

fn clip_dir() -> Option<std::path::PathBuf> {
    if cfg!(test) {
        return None;
    }
    let home = std::env::var("HOME").ok()?;
    Some(std::path::PathBuf::from(home).join(".local/state/mde/clips"))
}

fn strip_marks(text: &str) -> String {
    text.replace(['\u{1e}', '\u{1f}', '\u{1d}', '\u{1c}'], "")
}

fn uri_preview(text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .take(8)
        .collect();
    let Some(first) = lines.first() else {
        return "Files".into();
    };
    let name = file_name(first);
    if lines.len() == 1 {
        format!("File · {name}")
    } else {
        format!("Files · {name} +{}", lines.len() - 1)
    }
}

fn file_name(uri: &str) -> String {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    let path = path.split('?').next().unwrap_or(path);
    let base = path
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(path);
    let decoded = percent_decode(base);
    if decoded.is_empty() {
        "file".into()
    } else {
        decoded.chars().take(80).collect()
    }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn html_preview(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if ch == '>' {
            in_tag = false;
            continue;
        }
        if in_tag {
            continue;
        }
        out.push(ch);
        if out.chars().count() >= 80 {
            break;
        }
    }
    let text = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        "markup".into()
    } else {
        text
    }
}

fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn pin_path() -> Option<std::path::PathBuf> {
    if cfg!(test) {
        return None;
    }
    let home = std::env::var("HOME").ok()?;
    Some(std::path::PathBuf::from(home).join(".local/state/mde/clipboard-pins.txt"))
}

fn wl_copy_bytes(mime: Option<&str>, bytes: &[u8]) -> Result<(), String> {
    let mut command = Command::new("wl-copy");
    if let Some(mime) = mime {
        command.args(["-t", mime]);
    }
    let mut child = command
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("wl-copy: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(bytes)
            .map_err(|e| format!("wl-copy write: {e}"))?;
    }
    let status = child.wait().map_err(|e| format!("wl-copy wait: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("wl-copy exited {status}"))
    }
}

/// Best-effort HTML → plain for paste-as-text (W123).
fn strip_simple_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    let text = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        html.to_string()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respects_max() {
        let mut h = ClipboardHistory::new(true, 2);
        h.push("a");
        h.push("b");
        h.push("c");
        assert_eq!(h.items.len(), 2);
        assert_eq!(h.items[0].preview, "c");
        assert_eq!(h.items[0].full, "c");
    }

    #[test]
    fn pin_cap_is_ten() {
        let mut h = ClipboardHistory::new(true, 20);
        for i in 0..12 {
            h.push(&format!("p{i}"));
            let id = h.items[0].id;
            let ok = h.toggle_pin(id);
            if i < 10 {
                assert_eq!(ok, Some(true));
            } else {
                assert_eq!(ok, None);
            }
        }
        assert_eq!(h.items.iter().filter(|e| e.pinned).count(), 10);
    }

    #[test]
    fn pin_survives_trim_and_clear() {
        let mut h = ClipboardHistory::new(true, 2);
        h.push("keep");
        let id = h.items[0].id;
        assert_eq!(h.toggle_pin(id), Some(true));
        h.push("b");
        h.push("c");
        assert!(h.items.iter().any(|e| e.full == "keep" && e.pinned));
        assert_eq!(h.items.iter().filter(|e| !e.pinned).count(), 2);
        h.clear_unpinned();
        assert_eq!(h.items.len(), 1);
        assert_eq!(h.items[0].full, "keep");
    }

    #[test]
    fn html_is_kept_apart_from_plain_text() {
        let mut h = ClipboardHistory::new(true, 4);
        h.push("hello");
        assert!(h.push_html("<p>hello</p>"));
        assert_eq!(h.items[0].mime, "text/html");
        assert_eq!(h.items[0].preview, "HTML · hello");
        assert!(!h.push_html("<p>hello</p>"));
        assert_eq!(h.items.iter().filter(|e| e.mime == "text/html").count(), 1);
        assert!(h.items.iter().any(|e| e.full == "hello" && e.mime.is_empty()));
    }

    #[test]
    fn uri_list_is_its_own_clip() {
        let mut h = ClipboardHistory::new(true, 4);
        h.push("hello");
        assert!(h.push_uris("file:///home/mm/My%20Notes.txt\n"));
        assert_eq!(h.items[0].mime, "text/uri-list");
        assert_eq!(h.items[0].preview, "File · My Notes.txt");
        assert!(!h.push_uris("file:///home/mm/My%20Notes.txt\n"));
        assert!(h.push_uris("file:///tmp/a.png\nfile:///tmp/b.png\n"));
        assert_eq!(h.items[0].preview, "Files · a.png +1");
    }

    #[test]
    fn cross_device_import_skips_reexport() {
        let dir = std::env::temp_dir().join(format!("mde-clip-sync-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let remote = "hello from other device";
        let digest = fnv64(remote.as_bytes());
        std::fs::write(dir.join(format!("{digest:x}.txt")), remote).unwrap();
        let mut h = ClipboardHistory::with_sync(true, 8, true, Some(dir.clone()));
        h.poll_sync();
        assert!(h.items.iter().any(|e| e.full == remote));
        // Import path must not write a second sync file for the same text.
        let count = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("txt"))
            .count();
        assert_eq!(count, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
