//! Widgets Board cards (Win11-like) — weather, clock, system, tips.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct WeatherCard {
    pub place: String,
    pub summary: String,
    pub temp_c: String,
    pub fetched_at: Option<Instant>,
}

impl Default for WeatherCard {
    fn default() -> Self {
        Self {
            place: "Local".into(),
            summary: "Weather unavailable".into(),
            temp_c: "—".into(),
            fetched_at: None,
        }
    }
}

impl WeatherCard {
    pub fn needs_refresh(&self) -> bool {
        match self.fetched_at {
            None => true,
            Some(at) => at.elapsed() > Duration::from_secs(30 * 60),
        }
    }

    /// Best-effort fetch via `curl` + wttr.in (no new crate deps).
    /// On success, also writes `~/.local/state/mde/weather.cache` for mde-lock (W59).
    pub fn refresh(&mut self) {
        let Ok(out) = Command::new("curl")
            .args([
                "-fsSL",
                "--max-time",
                "4",
                "https://wttr.in/?format=%l|%C|%t",
            ])
            .output()
        else {
            return;
        };
        if !out.status.success() {
            return;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let parts: Vec<&str> = text.trim().split('|').collect();
        if parts.len() >= 3 {
            self.place = parts[0].trim().chars().take(40).collect();
            self.summary = parts[1].trim().chars().take(40).collect();
            self.temp_c = parts[2].trim().chars().take(12).collect();
            self.fetched_at = Some(Instant::now());
            write_weather_cache(&self.place, &self.summary, &self.temp_c);
        }
    }
}

/// Path for the small weather cache shared with mde-lock (W59).
pub fn weather_cache_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".local/state/mde/weather.cache"))
}

fn write_weather_cache(place: &str, summary: &str, temp_c: &str) {
    let Some(path) = weather_cache_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let line = format!("{place}|{summary}|{temp_c}\n");
    let _ = fs::write(path, line);
}

/// Read weather cache written by the shell; used by lock / soft-lock.
pub fn read_weather_cache_line() -> String {
    let Some(path) = weather_cache_path() else {
        return "Weather · see Widgets".into();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return "Weather · see Widgets".into();
    };
    let parts: Vec<&str> = text.trim().split('|').collect();
    if parts.len() >= 3 {
        let place = parts[0].trim().chars().take(32).collect::<String>();
        let summary = parts[1].trim().chars().take(24).collect::<String>();
        let temp = parts[2].trim().chars().take(12).collect::<String>();
        if !place.is_empty() && !temp.is_empty() {
            return format!("{place} · {summary} · {temp}");
        }
    }
    "Weather · see Widgets".into()
}

/// Temperature-only string when `weather.cache` is present (W137 taskbar glyph).
pub fn read_weather_cache_temp() -> Option<String> {
    let path = weather_cache_path()?;
    let text = fs::read_to_string(path).ok()?;
    let parts: Vec<&str> = text.trim().split('|').collect();
    if parts.len() >= 3 {
        let temp = parts[2].trim();
        if !temp.is_empty() && temp != "—" {
            return Some(temp.chars().take(12).collect());
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct SystemCard {
    pub load: String,
    pub memory: String,
    pub cpu_pct: String,
    pub fetched_at: Option<Instant>,
}

impl Default for SystemCard {
    fn default() -> Self {
        Self {
            load: "—".into(),
            memory: "—".into(),
            cpu_pct: "—".into(),
            fetched_at: None,
        }
    }
}

impl SystemCard {
    pub fn refresh(&mut self) {
        if let Ok(text) = fs::read_to_string("/proc/loadavg") {
            let load1 = text.split_whitespace().next().unwrap_or("—");
            self.load = format!("{load1} load");
        }
        if let Ok(text) = fs::read_to_string("/proc/meminfo") {
            let mut total_kb = 0u64;
            let mut avail_kb = 0u64;
            for line in text.lines() {
                if let Some(rest) = line.strip_prefix("MemTotal:") {
                    total_kb = rest.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(0);
                } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
                    avail_kb = rest.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(0);
                }
            }
            if total_kb > 0 {
                let used = total_kb.saturating_sub(avail_kb) as f64 / 1024.0 / 1024.0;
                let tot = total_kb as f64 / 1024.0 / 1024.0;
                self.memory = format!("{used:.1}/{tot:.0} GiB");
            }
        }
        // Idle-based CPU estimate from /proc/stat (instant snapshot vs previous not kept — coarse).
        if let Ok(text) = fs::read_to_string("/proc/stat") {
            if let Some(line) = text.lines().next() {
                let nums: Vec<u64> = line
                    .split_whitespace()
                    .skip(1)
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if nums.len() >= 4 {
                    let idle = nums[3];
                    let total: u64 = nums.iter().sum();
                    if total > 0 {
                        let busy = 100u64.saturating_sub(idle * 100 / total);
                        self.cpu_pct = format!("{busy}% cpu");
                    }
                }
            }
        }
        self.fetched_at = Some(Instant::now());
    }
}

/// Relative “Updated … ago” for Widgets cards (W99).
pub fn updated_ago(fetched_at: Option<Instant>) -> String {
    match fetched_at {
        None => "Updated · never".into(),
        Some(at) => {
            let secs = at.elapsed().as_secs();
            if secs < 60 {
                format!("Updated {secs}s ago")
            } else if secs < 3600 {
                format!("Updated {}m ago", secs / 60)
            } else if secs < 86400 {
                format!("Updated {}h ago", secs / 3600)
            } else {
                format!("Updated {}d ago", secs / 86400)
            }
        }
    }
}

/// Sorted image paths under ~/Pictures (jpg/png/webp) for Widgets photos (W52 / W239).
pub fn list_pictures() -> Vec<String> {
    let Ok(home) = std::env::var("HOME") else {
        return Vec::new();
    };
    let dir = PathBuf::from(home).join("Pictures");
    let Ok(rd) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut pics: Vec<String> = Vec::new();
    for ent in rd.flatten().take(200) {
        let path = ent.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") {
            pics.push(path.display().to_string());
        }
    }
    pics.sort();
    pics
}

/// Random image under ~/Pictures (jpg/png) for the Widgets photos card (W52).
pub fn random_picture() -> Option<String> {
    let pics = list_pictures();
    if pics.is_empty() {
        return None;
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Some(pics[(secs as usize / 300) % pics.len()].clone())
}

/// Next image in ~/Pictures after `current` (W239 cycle); wraps to first.
pub fn next_picture(current: Option<&str>) -> Option<String> {
    let pics = list_pictures();
    if pics.is_empty() {
        return None;
    }
    let Some(cur) = current else {
        return Some(pics[0].clone());
    };
    let idx = pics.iter().position(|p| p == cur).unwrap_or(0);
    Some(pics[(idx + 1) % pics.len()].clone())
}

const TIPS: &[&str] = &[
    "Tip: Super+L locks · type password to unlock",
    "Tip: Super+D shows the desktop",
    "Tip: Super+I opens Settings",
    "Tip: Super+Tab switches desktops",
    "Tip: Super+V opens clipboard history",
    "Tip: Super+Z picks a snap layout",
    "Tip: Super+W opens the Widgets board",
    "Tip: Super+. opens the emoji panel",
    "Tip: Far-right taskbar strip shows the desktop",
];

/// Hourly-rotating tip for the Widgets tip card (W15 / W199).
/// Reuses the same short Win11-style tips as the lock screen list.
pub fn tip_of_day() -> &'static str {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Rotate once per hour (W199).
    tip_at((secs / 3600) as usize)
}

/// Indexed tip for Start Tips row rotation (W233).
pub fn tip_at(idx: usize) -> &'static str {
    TIPS[idx % TIPS.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_card_reads_proc() {
        let mut c = SystemCard::default();
        c.refresh();
        assert!(!c.load.is_empty());
    }

    #[test]
    fn tip_nonempty() {
        assert!(!tip_of_day().is_empty());
    }
}


/// W435: local + UTC (+ optional extras `widgets.tz` IANA name) clock row.
pub fn timezone_row_label() -> String {
    use chrono::{Local, Utc};
    let local = Local::now().format("%H:%M").to_string();
    let utc = Utc::now().format("%H:%M").to_string();
    let extra = std::env::var("MDE_WIDGET_TZ").ok().filter(|s| !s.is_empty());
    // Prefer config via file if present later; env override for now.
    if let Some(tz) = extra {
        format!("Local {local} · UTC {utc} · {tz}")
    } else {
        format!("Local {local} · UTC {utc} · America/New_York")
    }
}

/// W430: News headlines from RSS/Atom (curl + light parse).
#[derive(Debug, Clone, Default)]
pub struct NewsCard {
    pub headlines: Vec<(String, String)>, // title, link
    pub source: String,
    pub fetched_at: Option<Instant>,
    pub error: String,
}

impl NewsCard {
    pub fn needs_refresh(&self) -> bool {
        match self.fetched_at {
            None => true,
            Some(at) => at.elapsed() > Duration::from_secs(15 * 60),
        }
    }

    pub fn refresh(&mut self, feed_url: &str) {
        self.error.clear();
        let Ok(out) = Command::new("curl")
            .args(["-fsSL", "--max-time", "8", "-A", "mde-shell/1.0", feed_url])
            .output()
        else {
            self.error = "curl failed".into();
            return;
        };
        if !out.status.success() {
            self.error = format!("HTTP {}", out.status);
            return;
        }
        let xml = String::from_utf8_lossy(&out.stdout);
        self.headlines = parse_rss_titles(&xml, 5);
        self.source = feed_url.chars().take(48).collect();
        self.fetched_at = Some(Instant::now());
        if self.headlines.is_empty() {
            self.error = "No items parsed".into();
        }
    }
}

fn parse_rss_titles(xml: &str, limit: usize) -> Vec<(String, String)> {
    let mut out = Vec::new();
    // Prefer <item>…</item> then <entry>…</entry>
    for block_tag in ["item", "entry"] {
        let open = format!("<{block_tag}");
        let close = format!("</{block_tag}>");
        let mut rest = xml;
        while out.len() < limit {
            let Some(start) = rest.find(&open) else {
                break;
            };
            let after = &rest[start..];
            let Some(end) = after.find(&close) else {
                break;
            };
            let block = &after[..end + close.len()];
            rest = &after[end + close.len()..];
            let title = xml_tag_text(block, "title").unwrap_or_default();
            let link = xml_tag_text(block, "link")
                .or_else(|| xml_attr_href(block))
                .unwrap_or_default();
            let title = html_decode(&strip_cdata(&title));
            if title.is_empty() {
                continue;
            }
            out.push((title.chars().take(80).collect(), link));
        }
        if !out.is_empty() {
            break;
        }
    }
    out
}

fn strip_cdata(s: &str) -> String {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix("<![CDATA[") {
        return inner.strip_suffix("]]>").unwrap_or(inner).trim().to_string();
    }
    s.to_string()
}

fn html_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

fn xml_tag_text(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = block.find(&open)?;
    let after_lt = &block[start..];
    let gt = after_lt.find('>')?;
    let content_start = start + gt + 1;
    let end = block[content_start..].find(&close)? + content_start;
    Some(block[content_start..end].trim().to_string())
}

fn xml_attr_href(block: &str) -> Option<String> {
    // Atom: <link href="..."/>
    let idx = block.find("href=\"")?;
    let rest = &block[idx + 6..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Default recommended feeds (subset of WORKLIST 20; first 6 enabled by default).
pub const DEFAULT_NEWS_FEEDS: &[(&str, &str)] = &[
    ("BBC News", "https://feeds.bbci.co.uk/news/rss.xml"),
    ("NPR News", "https://feeds.npr.org/1001/rss.xml"),
    ("The Guardian", "https://www.theguardian.com/world/rss"),
    ("TechCrunch", "https://techcrunch.com/feed/"),
    ("The Verge", "https://www.theverge.com/rss/index.xml"),
    ("Hacker News", "https://news.ycombinator.com/rss"),
];

/// W433: Markets watchlist quotes via Stooq CSV.
#[derive(Debug, Clone, Default)]
pub struct MarketsCard {
    pub rows: Vec<(String, String)>, // symbol, price
    pub fetched_at: Option<Instant>,
    pub error: String,
}

impl MarketsCard {
    pub fn needs_refresh(&self) -> bool {
        match self.fetched_at {
            None => true,
            Some(at) => at.elapsed() > Duration::from_secs(5 * 60),
        }
    }

    pub fn refresh(&mut self, symbols: &[String]) {
        self.error.clear();
        self.rows.clear();
        let syms: Vec<String> = if symbols.is_empty() {
            vec![
                "aapl.us".into(),
                "msft.us".into(),
                "googl.us".into(),
                "^spx".into(),
            ]
        } else {
            symbols.to_vec()
        };
        for sym in syms.iter().take(6) {
            let url = format!(
                "https://stooq.com/q/l/?s={}&f=sd2t2ohlcv&h&e=csv",
                sym.to_lowercase()
            );
            let Ok(out) = Command::new("curl")
                .args(["-fsSL", "--max-time", "6", &url])
                .output()
            else {
                continue;
            };
            if !out.status.success() {
                continue;
            }
            let text = String::from_utf8_lossy(&out.stdout);
            // CSV: Symbol,Date,Time,Open,High,Low,Close,...
            if let Some(line) = text.lines().nth(1) {
                let cols: Vec<&str> = line.split(',').collect();
                if cols.len() >= 7 {
                    let price = cols[6].trim();
                    if price != "N/D" && !price.is_empty() {
                        self.rows
                            .push((sym.to_uppercase(), format!("{price}")));
                    }
                }
            }
        }
        self.fetched_at = Some(Instant::now());
        if self.rows.is_empty() {
            self.error = "No quotes".into();
        }
    }
}

/// W431: Commute ETA via public OSRM.
#[derive(Debug, Clone, Default)]
pub struct TrafficCard {
    pub label: String,
    pub eta: String,
    pub detail: String,
    pub fetched_at: Option<Instant>,
    pub error: String,
}

impl TrafficCard {
    pub fn needs_refresh(&self) -> bool {
        match self.fetched_at {
            None => true,
            Some(at) => at.elapsed() > Duration::from_secs(10 * 60),
        }
    }

    /// `home` / `work` as "lon,lat" pairs (OSRM order).
    pub fn refresh(&mut self, home: &str, work: &str) {
        self.error.clear();
        if home.trim().is_empty() || work.trim().is_empty() {
            self.label = "Commute".into();
            self.eta = "—".into();
            self.detail = "Set widgets.traffic.home / .work (lon,lat)".into();
            self.error = "not configured".into();
            return;
        }
        let url = format!(
            "https://router.project-osrm.org/route/v1/driving/{home};{work}?overview=false"
        );
        let Ok(out) = Command::new("curl")
            .args(["-fsSL", "--max-time", "8", &url])
            .output()
        else {
            self.error = "curl failed".into();
            return;
        };
        if !out.status.success() {
            self.error = "OSRM error".into();
            return;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        // Parse "duration":123.4 without full JSON crate dance
        let duration = json_f64_field(&text, "duration").unwrap_or(0.0);
        let distance = json_f64_field(&text, "distance").unwrap_or(0.0);
        if duration <= 0.0 {
            self.error = "No route".into();
            self.detail = "Check lon,lat order (OSRM: lon,lat)".into();
            return;
        }
        let mins = (duration / 60.0).round() as i64;
        let km = distance / 1000.0;
        self.label = "Home → Work".into();
        self.eta = format!("{mins} min");
        self.detail = format!("{km:.1} km · OSRM");
        self.fetched_at = Some(Instant::now());
    }
}

fn json_f64_field(json: &str, key: &str) -> Option<f64> {
    let pat = format!("\"{key}\":");
    let idx = json.find(&pat)?;
    let rest = json[idx + pat.len()..].trim_start();
    let end = rest
        .find(|c: char| c == ',' || c == '}' || c == ']')
        .unwrap_or(rest.len());
    rest[..end].trim().parse().ok()
}
