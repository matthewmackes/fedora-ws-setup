//! Web enrichment for the podcast pages: show background and reviews (Apple
//! Podcasts, Podcast Index, Wikipedia), related shows, and per-episode chapters
//! and transcripts. Everything here is best effort and never fails the page.

use super::{
    apple_search, feed_key, http, http_json, index_credentials, index_get, strip_html, Episode,
    Show,
};
use anyhow::Result;
use serde_json::Value;
use std::io::Read;

#[derive(Debug, Clone, Default)]
pub struct Review {
    pub author: String,
    pub title: String,
    pub body: String,
    pub stars: u8,
}

#[derive(Debug, Clone, Default)]
pub struct ShowExtras {
    pub genre: String,
    pub categories: Vec<String>,
    /// Background text and the page it came from (Wikipedia).
    pub summary: Option<(String, String)>,
    /// Longer description from Podcast Index, when the feed's own is short.
    pub long_description: Option<String>,
    pub rating: Option<(f32, usize)>,
    pub reviews: Vec<Review>,
    pub related: Vec<Show>,
}

#[derive(Debug, Clone, Default)]
pub struct Chapter {
    pub start: u64,
    pub title: String,
}

#[derive(Debug, Clone, Default)]
pub struct TranscriptLine {
    pub start: Option<u64>,
    pub speaker: String,
    pub text: String,
}

pub fn show_extras(show: &Show) -> ShowExtras {
    let mut extras = ShowExtras::default();
    let apple_id = apple_id_for(show);
    if let Some(id) = apple_id.as_deref() {
        if let Ok(value) = http_json(&format!("https://itunes.apple.com/lookup?id={id}")) {
            if let Some(first) = value.pointer("/results/0") {
                extras.genre = first
                    .get("primaryGenreName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if let Some(genres) = first.get("genres").and_then(|v| v.as_array()) {
                    extras.categories = genres
                        .iter()
                        .filter_map(|g| g.as_str())
                        .filter(|g| *g != "Podcasts")
                        .map(str::to_string)
                        .collect();
                }
            }
        }
        reviews_for(id, &mut extras);
    }
    if let Some(desc) = index_description(show) {
        extras.long_description = Some(desc);
    }
    extras.summary = wikipedia_summary(&show.title);
    extras.related = related_shows(show, &extras.genre);
    extras
}

fn apple_id_for(show: &Show) -> Option<String> {
    if let Some(id) = show.id.strip_prefix("apple:") {
        return Some(id.to_string());
    }
    let key = feed_key(&show.feed_url);
    if key.is_empty() {
        return None;
    }
    apple_search(&show.title)
        .ok()?
        .into_iter()
        .find(|found| feed_key(&found.feed_url) == key)
        .and_then(|found| found.id.strip_prefix("apple:").map(str::to_string))
}

fn reviews_for(id: &str, extras: &mut ShowExtras) {
    let url = format!("https://itunes.apple.com/us/rss/customerreviews/id={id}/sortby=mostrecent/json");
    let Ok(value) = http_json(&url) else {
        return;
    };
    let entries = match value.pointer("/feed/entry") {
        Some(Value::Array(items)) => items.clone(),
        Some(one @ Value::Object(_)) => vec![one.clone()],
        _ => Vec::new(),
    };
    let label = |entry: &Value, path: &str| {
        entry
            .pointer(path)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let mut all = Vec::new();
    for entry in &entries {
        let stars: u8 = label(entry, "/im:rating/label").parse().unwrap_or(0);
        if stars == 0 {
            continue;
        }
        all.push(Review {
            author: label(entry, "/author/name/label"),
            title: label(entry, "/title/label"),
            body: label(entry, "/content/label"),
            stars,
        });
    }
    if !all.is_empty() {
        let sum: u32 = all.iter().map(|r| r.stars as u32).sum();
        extras.rating = Some((sum as f32 / all.len() as f32, all.len()));
    }
    all.retain(|r| r.body.chars().count() >= 40);
    all.sort_by(|a, b| b.stars.cmp(&a.stars).then(b.body.len().cmp(&a.body.len())));
    all.truncate(3);
    extras.reviews = all;
}

fn index_description(show: &Show) -> Option<String> {
    let (key, secret) = index_credentials()?;
    if show.feed_url.trim().is_empty() {
        return None;
    }
    let url = format!(
        "https://api.podcastindex.org/api/1.0/podcasts/byfeedurl?url={}",
        urlencoding::encode(show.feed_url.trim())
    );
    let value = index_get(&url, &key, &secret).ok()?;
    let desc = value.pointer("/feed/description")?.as_str()?;
    let desc = strip_html(desc);
    (desc.chars().count() > show.description.chars().count() + 40).then_some(desc)
}

/// A Wikipedia lead paragraph, only when the article is plainly about a podcast.
fn wikipedia_summary(title: &str) -> Option<(String, String)> {
    for candidate in [format!("{title} (podcast)"), title.to_string()] {
        let url = format!(
            "https://en.wikipedia.org/api/rest_v1/page/summary/{}",
            urlencoding::encode(&candidate.replace(' ', "_"))
        );
        let Ok(value) = http_json(&url) else {
            continue;
        };
        if value.get("type").and_then(|v| v.as_str()) != Some("standard") {
            continue;
        }
        let about = value
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let extract = value.get("extract").and_then(|v| v.as_str()).unwrap_or("");
        let podcast_like = about.contains("podcast")
            || about.contains("radio program")
            || extract.to_ascii_lowercase().contains("podcast");
        if !podcast_like || extract.chars().count() < 60 {
            continue;
        }
        let page = value
            .pointer("/content_urls/desktop/page")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        return Some((extract.to_string(), page));
    }
    None
}

fn related_shows(show: &Show, genre: &str) -> Vec<Show> {
    let own = feed_key(&show.feed_url);
    let mut out: Vec<Show> = Vec::new();
    let add = |found: Vec<Show>, out: &mut Vec<Show>| {
        for s in found {
            if feed_key(&s.feed_url) == own || s.id == show.id {
                continue;
            }
            if !out.iter().any(|have| feed_key(&have.feed_url) == feed_key(&s.feed_url)) {
                out.push(s);
            }
        }
    };
    if !show.author.trim().is_empty() {
        if let Ok(found) = apple_search(show.author.trim()) {
            add(found, &mut out);
        }
    }
    if out.len() < 8 && !genre.is_empty() {
        if let Ok(found) = apple_search(genre) {
            add(found, &mut out);
        }
    }
    out.truncate(10);
    out
}

pub fn chapters(episode: &Episode) -> Vec<Chapter> {
    if let Some(url) = episode.chapters_url.as_deref() {
        if let Ok(list) = chapters_from_json(url) {
            if !list.is_empty() {
                return list;
            }
        }
    }
    chapters_from_notes(&episode.description)
}

fn chapters_from_json(url: &str) -> Result<Vec<Chapter>> {
    let value = http_json(url)?;
    let list = value
        .get("chapters")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<Chapter> = list
        .iter()
        .filter_map(|c| {
            let start = c.get("startTime").and_then(|v| v.as_f64())?;
            let title = c.get("title").and_then(|v| v.as_str())?.trim().to_string();
            (!title.is_empty()).then_some(Chapter {
                start: start.max(0.0) as u64,
                title,
            })
        })
        .collect();
    out.sort_by_key(|c| c.start);
    Ok(out)
}

/// Timestamped lines in the show notes ("12:30 Topic"), kept only when they
/// read as a table of contents (3+ ascending entries).
pub fn chapters_from_notes(notes: &str) -> Vec<Chapter> {
    let text = block_text(notes);
    let mut out: Vec<Chapter> = Vec::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches(['-', '•', '*', '(', '[', ' ']);
        let end = line
            .find(|c: char| !(c.is_ascii_digit() || c == ':'))
            .unwrap_or(line.len());
        let (stamp, rest) = line.split_at(end);
        let Some(start) = super::parse_duration(stamp).filter(|_| stamp.contains(':')) else {
            continue;
        };
        let title = rest.trim_start_matches([')', ']', ' ', '-', '–', '—', ':']).trim();
        if title.is_empty() {
            continue;
        }
        if out.last().is_some_and(|last| start < last.start) {
            return Vec::new();
        }
        out.push(Chapter {
            start,
            title: title.to_string(),
        });
    }
    if out.len() >= 3 {
        out
    } else {
        Vec::new()
    }
}

pub fn transcript(episode: &Episode) -> Result<Vec<TranscriptLine>> {
    let Some(url) = episode.transcript_url.as_deref() else {
        anyhow::bail!("this episode has no transcript in its feed");
    };
    let mut bytes = Vec::new();
    http()
        .get(url)
        .call()?
        .into_reader()
        .take(4_000_000)
        .read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let kind = episode.transcript_type.as_deref().unwrap_or("");
    let mut lines = if kind.contains("json") || text.trim_start().starts_with('{') {
        transcript_json(&text)
    } else if kind.contains("vtt") || kind.contains("srt") || kind.contains("subrip") || text.contains("-->") {
        transcript_cues(&text)
    } else {
        block_text(&text)
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| TranscriptLine {
                text: l.to_string(),
                ..Default::default()
            })
            .collect()
    };
    lines.truncate(4000);
    if lines.is_empty() {
        anyhow::bail!("the transcript was empty");
    }
    Ok(lines)
}

fn transcript_json(text: &str) -> Vec<TranscriptLine> {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let segments = value
        .get("segments")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    segments
        .iter()
        .filter_map(|s| {
            let body = s.get("body").and_then(|v| v.as_str())?.trim().to_string();
            (!body.is_empty()).then(|| TranscriptLine {
                start: s.get("startTime").and_then(|v| v.as_f64()).map(|t| t.max(0.0) as u64),
                speaker: s.get("speaker").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                text: body,
            })
        })
        .collect()
}

fn transcript_cues(text: &str) -> Vec<TranscriptLine> {
    let mut out = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let Some((from, _)) = line.split_once("-->") else {
            continue;
        };
        let start = cue_secs(from);
        let mut body = Vec::new();
        while let Some(next) = lines.peek() {
            if next.trim().is_empty() {
                break;
            }
            body.push(strip_html(next));
            lines.next();
        }
        let body = body.join(" ");
        if !body.trim().is_empty() {
            out.push(TranscriptLine {
                start,
                speaker: String::new(),
                text: body.trim().to_string(),
            });
        }
    }
    out
}

fn cue_secs(stamp: &str) -> Option<u64> {
    let stamp = stamp.trim().replace(',', ".");
    let whole = stamp.split('.').next()?;
    super::parse_duration(whole)
}

/// HTML to plain text that keeps paragraph and line breaks.
pub fn block_text(html: &str) -> String {
    let mut spaced = html.to_string();
    for tag in ["<br>", "<br/>", "<br />", "</p>", "</li>", "</div>", "<BR>", "</P>", "</LI>"] {
        spaced = spaced.replace(tag, &format!("{tag}\n"));
    }
    let mut out = String::new();
    let mut in_tag = false;
    for ch in spaced.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&amp;", "&").replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapters_come_from_timestamped_notes() {
        let notes = "<p>Intro</p><p>00:00 Cold open</p><p>03:15 The news</p><p>1:02:10 Wrap up</p>";
        let list = chapters_from_notes(notes);
        assert_eq!(list.len(), 3);
        assert_eq!(list[1].start, 195);
        assert_eq!(list[2].title, "Wrap up");
        assert!(chapters_from_notes("<p>00:10 only one</p>").is_empty());
    }

    #[test]
    fn vtt_cues_are_timed() {
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nHello <b>there</b>\n\n00:01:05.500 --> 00:01:07.000\nSecond";
        let lines = transcript_cues(vtt);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].start, Some(1));
        assert_eq!(lines[0].text, "Hello there");
        assert_eq!(lines[1].start, Some(65));
    }
}
