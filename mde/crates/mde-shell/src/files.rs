//! Filename search for Start/Search (Desktop, Documents, Downloads, Pictures, Music, Videos).

use std::path::PathBuf;
use std::process::Command;

pub fn search_files(query: &str, limit: usize) -> Vec<String> {
    let q = query.trim().to_lowercase();
    if q.len() < 2 {
        return Vec::new();
    }
    let Ok(home) = std::env::var("HOME") else {
        return Vec::new();
    };
    let roots = ["Desktop", "Documents", "Downloads", "Pictures", "Music", "Videos"];
    let mut hits = Vec::new();
    for root in roots {
        walk(PathBuf::from(&home).join(root), &q, 0, limit, &mut hits);
        if hits.len() >= limit {
            break;
        }
    }
    hits
}

fn walk(dir: PathBuf, query: &str, depth: u8, limit: usize, hits: &mut Vec<String>) {
    if depth > 2 || hits.len() >= limit {
        return;
    }
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return;
    };
    for ent in rd.flatten().take(80) {
        if hits.len() >= limit {
            return;
        }
        let path = ent.path();
        let name = ent.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if name.to_lowercase().contains(query) && path.is_file() {
            hits.push(path.display().to_string());
        }
        if path.is_dir() {
            walk(path, query, depth + 1, limit, hits);
        }
    }
}

pub fn open_path(path: &str) -> Result<(), String> {
    // A044: prefer Thunar as MDE Explorer when available.
    if Command::new("thunar").arg(path).spawn().is_ok() {
        return Ok(());
    }
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|e| format!("xdg-open: {e}"))?;
    Ok(())
}

/// Content search over Desktop / Documents / Downloads only (W406 / A085).
/// Prefers `rg`, falls back to `grep -RIl`. Caps results; skips huge / binary paths via tool flags.
pub fn search_file_contents(query: &str, limit: usize) -> Vec<String> {
    let q = query.trim();
    if q.len() < 2 || limit == 0 {
        return Vec::new();
    }
    let Ok(home) = std::env::var("HOME") else {
        return Vec::new();
    };
    let roots: Vec<PathBuf> = ["Desktop", "Documents", "Downloads"]
        .iter()
        .map(|r| PathBuf::from(&home).join(r))
        .filter(|p| p.is_dir())
        .collect();
    if roots.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    if let Some(found) = rg_content(&roots, q, limit) {
        hits = found;
    } else if let Some(found) = grep_content(&roots, q, limit) {
        hits = found;
    }
    hits
}

fn rg_content(roots: &[PathBuf], query: &str, limit: usize) -> Option<Vec<String>> {
    let mut cmd = Command::new("rg");
    cmd.args([
        "-l",
        "-i",
        "--no-messages",
        "--hidden",
        "--max-filesize",
        "512K",
        "--max-count",
        "1",
        "-g",
        "!.*",
        "-g",
        "!*.png",
        "-g",
        "!*.jpg",
        "-g",
        "!*.jpeg",
        "-g",
        "!*.gif",
        "-g",
        "!*.webp",
        "-g",
        "!*.pdf",
        "-g",
        "!*.zip",
        "-g",
        "!*.tar*",
        "--",
        query,
    ]);
    for r in roots {
        cmd.arg(r);
    }
    let out = cmd.output().ok()?;
    if !out.status.success() && out.stdout.is_empty() {
        // exit 1 = no matches; other failures → None so grep can try
        if out.status.code() != Some(1) {
            return None;
        }
    }
    let mut hits = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let path = line.trim();
        if path.is_empty() {
            continue;
        }
        hits.push(path.to_string());
        if hits.len() >= limit {
            break;
        }
    }
    Some(hits)
}

fn grep_content(roots: &[PathBuf], query: &str, limit: usize) -> Option<Vec<String>> {
    let mut hits = Vec::new();
    for root in roots {
        if hits.len() >= limit {
            break;
        }
        let out = Command::new("grep")
            .args([
                "-RIl",
                "-i",
                "--exclude-dir=.git",
                "--exclude-dir=.cache",
                "--exclude=*.png",
                "--exclude=*.jpg",
                "--exclude=*.jpeg",
                "--exclude=*.gif",
                "--exclude=*.webp",
                "--exclude=*.pdf",
                "-e",
                query,
            ])
            .arg(root)
            .output()
            .ok()?;
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let path = line.trim();
            if path.is_empty() {
                continue;
            }
            if hits.iter().any(|h: &String| h == path) {
                continue;
            }
            hits.push(path.to_string());
            if hits.len() >= limit {
                break;
            }
        }
    }
    Some(hits)
}

/// Prefer Documents, then home — MDE Explorer via Thunar (W50 / A044).
pub fn open_file_explorer() -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME unset".to_string())?;
    let docs = xdg_user_dir("DOCUMENTS").unwrap_or_else(|| PathBuf::from(&home).join("Documents"));
    let target = if docs.is_dir() {
        docs
    } else {
        PathBuf::from(&home)
    };
    let shown = target.display().to_string();
    open_path(&shown)?;
    Ok(shown)
}

/// Open a well-known user folder (Documents / Pictures / Downloads) (W60).
pub fn open_user_folder(kind: &str) -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME unset".to_string())?;
    let (xdg, fallback) = match kind.to_ascii_lowercase().as_str() {
        "documents" | "docs" => ("DOCUMENTS", "Documents"),
        "pictures" | "pics" => ("PICTURES", "Pictures"),
        "downloads" | "dl" => ("DOWNLOADS", "Downloads"),
        "music" => ("MUSIC", "Music"),
        "desktop" => ("DESKTOP", "Desktop"),
        "home" => {
            let shown = home.clone();
            open_path(&shown)?;
            return Ok(shown);
        }
        _ => return Err(format!("unknown folder: {kind}")),
    };
    let target = xdg_user_dir(xdg).unwrap_or_else(|| PathBuf::from(&home).join(fallback));
    let shown = if target.is_dir() {
        target.display().to_string()
    } else {
        home
    };
    open_path(&shown)?;
    Ok(shown)
}

fn xdg_user_dir(name: &str) -> Option<PathBuf> {
    let out = Command::new("xdg-user-dir").arg(name).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

/// Resolve the XDG Desktop directory (`xdg-user-dir DESKTOP` or ~/Desktop).
pub fn desktop_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(xdg_user_dir("DESKTOP").unwrap_or_else(|| PathBuf::from(&home).join("Desktop")))
}

/// Folders + `.desktop` shortcuts on ~/Desktop for Start “Desktop” (W45 / W417–W419).
/// `sort` is `"name"` (default) or `"date"` (mtime descending).
pub fn desktop_folder_entries(limit: usize, sort: &str) -> Vec<(String, String)> {
    let Some(dir) = desktop_dir() else {
        return Vec::new();
    };
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut entries: Vec<(String, String, u64)> = Vec::new();
    for ent in rd.flatten() {
        let path = ent.path();
        let name = ent.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let meta = ent.metadata().ok();
        let mtime = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if path.is_dir() {
            entries.push((name.into_owned(), path.display().to_string(), mtime));
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let label = apps_desktop_name(&path).unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string())
        });
        entries.push((label, path.display().to_string(), mtime));
    }
    if sort.eq_ignore_ascii_case("date") {
        entries.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
    } else {
        entries.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    }
    entries
        .into_iter()
        .take(limit)
        .map(|(label, path, _)| (label, path))
        .collect()
}

/// W419: mkdir a uniquely named folder under ~/Desktop.
pub fn desktop_new_folder() -> Result<String, String> {
    let dir = desktop_dir().ok_or_else(|| "Desktop · no HOME".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Desktop · {e}"))?;
    let path = unique_child(&dir, "New Folder", false);
    std::fs::create_dir(&path).map_err(|e| format!("New Folder · {e}"))?;
    Ok(path.display().to_string())
}

/// W419: create a stub `.desktop` shortcut under ~/Desktop.
pub fn desktop_new_shortcut() -> Result<String, String> {
    let dir = desktop_dir().ok_or_else(|| "Desktop · no HOME".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Desktop · {e}"))?;
    let path = unique_child(&dir, "New Shortcut.desktop", true);
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
    let body = format!(
        "[Desktop Entry]\nType=Application\nName=New Shortcut\nComment=Created by MDE\nExec=xdg-open {home}\nIcon=application-x-executable\nTerminal=false\n"
    );
    std::fs::write(&path, body).map_err(|e| format!("New Shortcut · {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
    }
    Ok(path.display().to_string())
}

fn unique_child(dir: &std::path::Path, base: &str, is_file: bool) -> PathBuf {
    let candidate = dir.join(base);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = if is_file {
        if let Some(dot) = base.rfind('.') {
            (&base[..dot], &base[dot..])
        } else {
            (base, "")
        }
    } else {
        (base, "")
    };
    for i in 2..1000 {
        let name = format!("{stem} ({i}){ext}");
        let p = dir.join(name);
        if !p.exists() {
            return p;
        }
    }
    dir.join(format!("{stem}-{}", std::process::id()))
}

fn apps_desktop_name(path: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Name=") {
            let name = rest.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// W458: lightweight name index under XDG cache (idle refresh).
pub fn file_index_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let cache = std::env::var("XDG_CACHE_HOME").unwrap_or_else(|_| format!("{home}/.cache"));
    Some(PathBuf::from(cache).join("mde").join("file-index.txt"))
}

pub fn refresh_file_index(limit: usize) -> usize {
    let Some(path) = file_index_path() else {
        return 0;
    };
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new(".")));
    let Ok(home) = std::env::var("HOME") else {
        return 0;
    };
    let mut names = Vec::new();
    for root in ["Desktop", "Documents", "Downloads", "Pictures", "Music", "Videos"] {
        walk_index(PathBuf::from(&home).join(root), 0, limit, &mut names);
        if names.len() >= limit {
            break;
        }
    }
    let n = names.len();
    let _ = std::fs::write(&path, names.join("\n"));
    n
}

fn walk_index(dir: PathBuf, depth: u8, limit: usize, out: &mut Vec<String>) {
    if depth > 3 || out.len() >= limit {
        return;
    }
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return;
    };
    for ent in rd.flatten().take(100) {
        if out.len() >= limit {
            return;
        }
        let path = ent.path();
        let name = ent.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if path.is_file() {
            out.push(path.display().to_string());
        } else if path.is_dir() {
            walk_index(path, depth + 1, limit, out);
        }
    }
}

pub fn search_file_index(query: &str, limit: usize) -> Vec<String> {
    let q = query.trim().to_lowercase();
    if q.len() < 2 {
        return Vec::new();
    }
    let Some(path) = file_index_path() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| line.to_lowercase().contains(&q))
        .take(limit)
        .map(|s| s.to_string())
        .collect()
}
