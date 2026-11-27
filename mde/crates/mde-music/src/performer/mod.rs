//! Performer overview — MusicBrainz (+ Wikipedia blurb) with 7-day disk cache.

use anyhow::Result;
use md5::Digest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const UA: &str = "NeighbourhoodMusic/2026 (MDE; https://localhost)";
const CACHE_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
const CONFIDENT: f64 = 0.78;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerformerCandidate {
    pub mbid: String,
    pub name: String,
    pub disambiguation: String,
    pub score: f64,
    pub typ: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerformerInfo {
    pub mbid: String,
    pub name: String,
    pub disambiguation: String,
    pub typ: String,
    pub gender: String,
    pub country: String,
    pub begin: String,
    pub end: String,
    pub area: String,
    pub genres: Vec<String>,
    pub members: Vec<String>,
    pub bio: String,
    #[serde(default)]
    pub wiki_title: String,
    pub fanart_thumb: Option<String>,
    pub fanart_background: Option<String>,
    pub confirmed: bool,
}

fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neighbourhood-music")
        .join("performers")
}

fn cache_path(key: &str) -> PathBuf {
    let digest = md5::Md5::digest(key.as_bytes());
    cache_dir().join(format!("{}.json", hex::encode(digest)))
}

fn load_cache(key: &str) -> Option<PerformerInfo> {
    let path = cache_path(key);
    let meta = fs::metadata(&path).ok()?;
    let modified = meta.modified().ok()?;
    if SystemTime::now().duration_since(modified).ok()? > CACHE_TTL {
        return None;
    }
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn save_cache(key: &str, info: &PerformerInfo) {
    let _ = fs::create_dir_all(cache_dir());
    if let Ok(text) = serde_json::to_string(info) {
        let _ = fs::write(cache_path(key), text);
    }
}

fn remembered_mbid(artist_id: &str) -> Option<String> {
    let key = format!("confirm:{artist_id}");
    fs::read_to_string(cache_path(&key))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn remember_mbid(artist_id: &str, mbid: &str) {
    let key = format!("confirm:{artist_id}");
    let _ = fs::create_dir_all(cache_dir());
    let _ = fs::write(cache_path(&key), mbid);
}

pub fn search_artists(name: &str) -> Result<Vec<PerformerCandidate>> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!(
        "https://musicbrainz.org/ws/2/artist/?query=artist:{}&fmt=json&limit=8",
        urlencoding::encode(&format!("\"{name}\""))
    );
    let resp = ureq::get(&url)
        .set("User-Agent", UA)
        .set("Accept", "application/json")
        .timeout(Duration::from_secs(8))
        .call()?;
    let v: Value = resp.into_json()?;
    let mut out = Vec::new();
    if let Some(arr) = v.get("artists").and_then(|a| a.as_array()) {
        for a in arr {
            let mbid = a.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
            if mbid.is_empty() {
                continue;
            }
            let score = a.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0) / 100.0;
            out.push(PerformerCandidate {
                mbid,
                name: a
                    .get("name")
                    .and_then(|x| x.as_str())
                    .unwrap_or(name)
                    .to_string(),
                disambiguation: a
                    .get("disambiguation")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
                score,
                typ: a
                    .get("type")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(out)
}

pub fn fetch_artist(mbid: &str) -> Result<PerformerInfo> {
    let cache_key = format!("mbid2:{mbid}");
    if let Some(hit) = load_cache(&cache_key) {
        return Ok(hit);
    }
    let url = format!(
        "https://musicbrainz.org/ws/2/artist/{mbid}?inc=genres+artist-rels+url-rels&fmt=json"
    );
    let resp = ureq::get(&url)
        .set("User-Agent", UA)
        .set("Accept", "application/json")
        .timeout(Duration::from_secs(10))
        .call()?;
    let v: Value = resp.into_json()?;
    let mut info = PerformerInfo {
        mbid: mbid.to_string(),
        name: v
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        disambiguation: v
            .get("disambiguation")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        typ: v
            .get("type")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        gender: v
            .get("gender")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        country: v
            .get("country")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        begin: v
            .pointer("/life-span/begin")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        end: v
            .pointer("/life-span/end")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        area: v
            .pointer("/area/name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        genres: v
            .get("genres")
            .and_then(|g| g.as_array())
            .map(|arr| {
                let mut g: Vec<String> = arr
                    .iter()
                    .filter_map(|x| x.get("name").and_then(|n| n.as_str()).map(str::to_string))
                    .collect();
                g.sort();
                g.dedup();
                g.into_iter().take(8).collect()
            })
            .unwrap_or_default(),
        members: {
            let mut m = Vec::new();
            if let Some(rels) = v.get("relations").and_then(|r| r.as_array()) {
                for r in rels {
                    let typ = r.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    if matches!(typ, "member of band" | "vocal" | "instrument") {
                        if let Some(name) = r.pointer("/artist/name").and_then(|n| n.as_str()) {
                            m.push(name.to_string());
                        }
                    }
                }
            }
            m.sort();
            m.dedup();
            m.into_iter().take(12).collect()
        },
        bio: String::new(),
        wiki_title: String::new(),
        fanart_thumb: None,
        fanart_background: None,
        confirmed: true,
    };

    if let Some((title, text)) = wikipedia_blurb_from_relations(&v) {
        info.bio = text;
        info.wiki_title = title;
    }
    if let Some((thumb, bg)) = crate::artwork::fanart_for_mbid(mbid) {
        info.fanart_thumb = thumb;
        info.fanart_background = bg;
    }

    save_cache(&cache_key, &info);
    Ok(info)
}

fn wikipedia_blurb_from_relations(v: &Value) -> Option<(String, String)> {
    let rels = v.get("relations")?.as_array()?;
    let mut title: Option<String> = None;
    for r in rels {
        let typ = r.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if typ != "wikipedia" && typ != "wikidata" {
            continue;
        }
        let url = r
            .pointer("/url/resource")
            .and_then(|u| u.as_str())
            .unwrap_or("");
        if url.contains("wikipedia.org/wiki/") {
            title = url.rsplit('/').next().map(|s| {
                urlencoding::decode(s)
                    .map(|c| c.into_owned())
                    .unwrap_or_else(|_| s.to_string())
            });
            break;
        }
        if typ == "wikidata" {
            if let Some(qid) = url.rsplit('/').next() {
                if let Some(t) = wikidata_enwiki_title(qid) {
                    title = Some(t);
                    break;
                }
            }
        }
    }
    let title = title?;
    // Whole lead section as plain text (not the 1–2 sentence summary).
    let api = format!(
        "https://en.wikipedia.org/w/api.php?action=query&prop=extracts&exintro=1&explaintext=1&redirects=1&format=json&titles={}",
        urlencoding::encode(&title)
    );
    let resp = ureq::get(&api)
        .set("User-Agent", UA)
        .timeout(Duration::from_secs(8))
        .call()
        .ok()?;
    let j: Value = resp.into_json().ok()?;
    let text = j
        .pointer("/query/pages")
        .and_then(|p| p.as_object())
        .and_then(|m| m.values().next())
        .and_then(|pg| pg.get("extract"))
        .and_then(|e| e.as_str())
        .map(clip_bio)?;
    (!text.is_empty()).then_some((title, text))
}

/// Keep whole paragraphs/sentences up to ~1800 characters.
pub fn clip_bio(extract: &str) -> String {
    let mut out = String::new();
    for para in extract.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        if out.chars().count() + para.chars().count() <= 1800 {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(para);
            continue;
        }
        if out.is_empty() {
            // One huge paragraph: cut at a sentence end.
            let cut: String = para.chars().take(1800).collect();
            let end = cut.rfind(". ").map(|i| i + 1).unwrap_or(cut.len());
            out.push_str(cut[..end].trim());
        }
        break;
    }
    out
}

fn wikidata_enwiki_title(qid: &str) -> Option<String> {
    let url = format!("https://www.wikidata.org/wiki/Special:EntityData/{qid}.json");
    let resp = ureq::get(&url)
        .set("User-Agent", UA)
        .timeout(Duration::from_secs(6))
        .call()
        .ok()?;
    let v: Value = resp.into_json().ok()?;
    v.pointer(&format!("/entities/{qid}/sitelinks/enwiki/title"))
        .and_then(|t| t.as_str())
        .map(str::to_string)
}

pub enum ResolveOutcome {
    Ready(PerformerInfo),
    NeedsConfirm(Vec<PerformerCandidate>),
    Empty,
}

pub fn resolve_performer(artist_id: &str, artist_name: &str) -> ResolveOutcome {
    if let Some(mbid) = remembered_mbid(artist_id) {
        if let Ok(info) = fetch_artist(&mbid) {
            return ResolveOutcome::Ready(info);
        }
    }
    let cache_key = format!("name2:{}", artist_name.trim().to_ascii_lowercase());
    if let Some(hit) = load_cache(&cache_key) {
        return ResolveOutcome::Ready(hit);
    }
    let Ok(cands) = search_artists(artist_name) else {
        return ResolveOutcome::Empty;
    };
    if cands.is_empty() {
        return ResolveOutcome::Empty;
    }
    if cands[0].score >= CONFIDENT {
        match fetch_artist(&cands[0].mbid) {
            Ok(mut info) => {
                info.confirmed = true;
                save_cache(&cache_key, &info);
                ResolveOutcome::Ready(info)
            }
            Err(_) => ResolveOutcome::NeedsConfirm(cands),
        }
    } else {
        ResolveOutcome::NeedsConfirm(cands)
    }
}

#[cfg(test)]
mod bio_tests {
    use super::clip_bio;

    #[test]
    fn keeps_whole_paragraphs_under_the_cap() {
        let a = "A".repeat(900);
        let b = "B".repeat(900);
        let c = "C".repeat(900);
        let t = clip_bio(&format!("{a}\n\n{b}\n\n{c}"));
        assert!(t.contains(&a) && t.contains(&b) && !t.contains(&c));
    }

    #[test]
    fn cuts_one_long_paragraph_at_a_sentence() {
        let para = format!("{} End here. {}", "word ".repeat(300), "tail ".repeat(400));
        let t = clip_bio(&para);
        assert!(t.chars().count() <= 1800);
        assert!(t.ends_with('.'), "{t}");
    }
}
