//! Episode guests: who is on an episode, and a short biography for each.
//!
//! Guests come from the Podcasting 2.0 `<podcast:person>` tag when the feed has one,
//! otherwise from conservative patterns in the episode title and description
//! ("with Jane Doe", "featuring …", "joined by …"). Biographies come from Wikipedia
//! and are cached on disk; a name that does not clearly match a Wikipedia article gets
//! no biography rather than a wrong one.

use anyhow::Result;
use md5::Digest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const UA: &str = "NeighbourhoodMusic/2026 (MDE; podcast guests)";
const HIT_TTL: Duration = Duration::from_secs(30 * 24 * 3600);
const MISS_TTL: Duration = Duration::from_secs(3 * 24 * 3600);
const MAX_GUESTS: usize = 4;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guest {
    pub name: String,
    #[serde(default)]
    pub role: String,
    /// Photo URL supplied by the feed (`img` attribute), if any.
    #[serde(default)]
    pub img: Option<String>,
    /// Profile link supplied by the feed (`href` attribute), if any.
    #[serde(default)]
    pub href: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuestBio {
    pub name: String,
    pub article: String,
    pub descriptor: String,
    pub bio: String,
    pub photo: Option<String>,
    pub url: String,
}

/// Guests from a feed item's `<podcast:person>` tags (hosts are skipped).
pub fn guests_from_extensions(item: &rss::Item) -> Vec<Guest> {
    let mut out = Vec::new();
    for ns in item.extensions().values() {
        let Some(people) = ns.get("person") else {
            continue;
        };
        for person in people {
            let name = person.value().unwrap_or("").trim().to_string();
            if name.is_empty() {
                continue;
            }
            let role = person
                .attrs()
                .get("role")
                .map(|r| r.trim().to_ascii_lowercase())
                .unwrap_or_else(|| "host".into());
            if role != "guest" {
                continue;
            }
            let attr = |key: &str| {
                person
                    .attrs()
                    .get(key)
                    .map(|v| v.trim().to_string())
                    .filter(|v| v.contains("://"))
            };
            push_unique(
                &mut out,
                Guest {
                    name,
                    role,
                    img: attr("img"),
                    href: attr("href"),
                },
            );
        }
    }
    out.truncate(MAX_GUESTS);
    out
}

/// Guests named in an episode title/description. `skip` lists names to ignore
/// (the show's own host/author).
pub fn guests_from_text(title: &str, description: &str, skip: &str) -> Vec<Guest> {
    let mut names = Vec::new();
    for marker in TITLE_MARKERS {
        for name in names_after(title, marker) {
            names.push(name);
        }
    }
    let desc: String = description.chars().take(700).collect();
    for marker in DESC_MARKERS {
        for name in names_after(&desc, marker) {
            names.push(name);
        }
    }
    let skip = skip.to_ascii_lowercase();
    let mut out = Vec::new();
    for name in names {
        if !skip.is_empty() && skip.contains(&name.to_ascii_lowercase()) {
            continue;
        }
        push_unique(
            &mut out,
            Guest {
                name,
                role: "guest".into(),
                ..Default::default()
            },
        );
    }
    out.truncate(MAX_GUESTS);
    out
}

const TITLE_MARKERS: &[&str] = &[
    " with ",
    " w/ ",
    " feat. ",
    " feat ",
    " featuring ",
    " ft. ",
    "guest: ",
    "guests: ",
    "interview: ",
    "interview with ",
    "conversation with ",
    "talks with ",
    "talks to ",
    "speaks with ",
];

const DESC_MARKERS: &[&str] = &[
    "joined by ",
    "interview with ",
    "conversation with ",
    "talks with ",
    "talks to ",
    "speaks with ",
    "featuring ",
    "guest: ",
    "guests: ",
    "my guest is ",
    "our guest is ",
    "this week's guest is ",
];

/// Candidate names following `marker` (case-insensitive) in `text`.
fn names_after(text: &str, marker: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = lower[from..].find(marker) {
        let start = from + at + marker.len();
        from = start;
        // `to_ascii_lowercase` keeps byte offsets identical to `text`.
        let rest = &text[start..];
        let end = rest
            .find(|c: char| matches!(c, '|' | '—' | '–' | '(' | '[' | ':' | '?' | '!' | ';' | '.'))
            .unwrap_or(rest.len());
        let mut chunk = &rest[..end];
        // "Jane Doe on climate" / "Jane Doe about X" / "Jane Doe - topic"
        for stop in [" on ", " about ", " - ", " and the ", " to ", " who ", " for ", " in "] {
            if let Some(i) = chunk.to_ascii_lowercase().find(stop) {
                chunk = &chunk[..i];
            }
        }
        for part in chunk.split(',').flat_map(|p| p.split(" & ")).flat_map(|p| p.split(" and ")) {
            if let Some(name) = clean_name(part) {
                found.push(name);
            }
        }
    }
    found
}

const PARTICLES: &[&str] = &["de", "da", "del", "van", "von", "bin", "al", "el", "la", "le", "di", "du", "st"];
const NOT_NAMES: &[&str] = &[
    "Episode", "Part", "Season", "Special", "Live", "Show", "Podcast", "Friends", "Family",
    "Others", "More", "Us", "You", "Me", "Him", "Her", "Them",
];

fn clean_name(raw: &str) -> Option<String> {
    let raw = raw
        .trim()
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '“' | '”' | '‘' | '’' | ',' | ' '));
    let words: Vec<&str> = raw.split_whitespace().collect();
    if !(2..=4).contains(&words.len()) {
        return None;
    }
    for (i, word) in words.iter().enumerate() {
        let bare = word.trim_end_matches('.');
        if i > 0 && i + 1 < words.len() && PARTICLES.contains(&bare.to_ascii_lowercase().as_str()) {
            continue;
        }
        let mut chars = bare.chars();
        let first = chars.next()?;
        if !first.is_uppercase() {
            return None;
        }
        if !bare
            .chars()
            .all(|c| c.is_alphabetic() || matches!(c, '\'' | '’' | '-'))
        {
            return None;
        }
        if NOT_NAMES.contains(&bare) {
            return None;
        }
    }
    Some(words.join(" "))
}

fn push_unique(out: &mut Vec<Guest>, guest: Guest) {
    if !out.iter().any(|g| g.name.eq_ignore_ascii_case(&guest.name)) {
        out.push(guest);
    }
}

// ───────────────────────── biography lookup ─────────────────────────

fn cache_path(name: &str) -> PathBuf {
    let digest = md5::Md5::digest(name.trim().to_lowercase().as_bytes());
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neighbourhood-music")
        .join("podcast-guests")
        .join(format!("{}.json", hex::encode(digest)))
}

fn load_cache(name: &str) -> Option<Option<GuestBio>> {
    let path = cache_path(name);
    let age = SystemTime::now()
        .duration_since(fs::metadata(&path).ok()?.modified().ok()?)
        .ok()?;
    let bio: Option<GuestBio> = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    let ttl = if bio.is_some() { HIT_TTL } else { MISS_TTL };
    (age <= ttl).then_some(bio)
}

fn save_cache(name: &str, bio: &Option<GuestBio>) {
    let path = cache_path(name);
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string(bio) {
        let _ = fs::write(path, text);
    }
}

/// Look up a short biography. `Ok(None)` means "no confident match";
/// `Err` means the lookup could not be completed (offline) and is not cached.
pub fn lookup_bio(name: &str) -> Result<Option<GuestBio>> {
    if let Some(hit) = load_cache(name) {
        return Ok(hit);
    }
    let api = format!(
        "https://en.wikipedia.org/w/api.php?action=query&generator=search&gsrsearch={}&gsrlimit=4\
         &prop=extracts%7Cpageimages%7Cdescription&exintro=1&explaintext=1&exsentences=6\
         &piprop=thumbnail&pithumbsize=360&redirects=1&format=json",
        urlencoding::encode(name)
    );
    let value: Value = ureq::get(&api)
        .set("User-Agent", UA)
        .timeout(Duration::from_secs(8))
        .call()?
        .into_json()?;
    let bio = pick_page(name, &value);
    save_cache(name, &bio);
    Ok(bio)
}

fn pick_page(name: &str, value: &Value) -> Option<GuestBio> {
    let pages = value.pointer("/query/pages")?.as_object()?;
    let mut ranked: Vec<&Value> = pages.values().collect();
    ranked.sort_by_key(|p| p.get("index").and_then(Value::as_i64).unwrap_or(99));
    let want = tokens(name);
    for page in ranked {
        let title = page.get("title").and_then(Value::as_str).unwrap_or("");
        let extract = page.get("extract").and_then(Value::as_str).unwrap_or("").trim();
        if extract.chars().count() < 40 || extract.contains("may refer to") {
            continue;
        }
        let have = tokens(title.split(" (").next().unwrap_or(title));
        // Every word of the guest's name must appear in the article title.
        if want.is_empty() || !want.iter().all(|w| have.contains(w)) {
            continue;
        }
        let descriptor = page
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        return Some(GuestBio {
            name: name.to_string(),
            article: title.to_string(),
            descriptor,
            bio: crate::performer::clip_bio(extract),
            photo: page
                .pointer("/thumbnail/source")
                .and_then(Value::as_str)
                .map(str::to_string),
            url: format!(
                "https://en.wikipedia.org/wiki/{}",
                title.replace(' ', "_")
            ),
        });
    }
    None
}

fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1)
        .map(|w| w.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(title: &str, desc: &str) -> Vec<String> {
        guests_from_text(title, desc, "")
            .into_iter()
            .map(|g| g.name)
            .collect()
    }

    #[test]
    fn title_patterns_find_guests() {
        assert_eq!(names("The Future of AI with Fei-Fei Li", ""), ["Fei-Fei Li"]);
        assert_eq!(
            names("Ep 12: Markets feat. Jane Doe and John Smith", ""),
            ["Jane Doe", "John Smith"]
        );
        assert_eq!(names("Interview with Ludwig van Beethoven | Live", ""), ["Ludwig van Beethoven"]);
    }

    #[test]
    fn ordinary_titles_find_nobody() {
        assert!(names("Living with the weather", "").is_empty());
        assert!(names("Why we sleep", "A look at rest.").is_empty());
        assert!(names("Dinner with Friends", "").is_empty());
    }

    #[test]
    fn description_needs_a_strong_marker() {
        assert_eq!(
            names("Episode 9", "This week the host is joined by Maya Angelou to talk poetry."),
            ["Maya Angelou"]
        );
        assert!(names("Episode 9", "We went with the plan we had.").is_empty());
    }

    #[test]
    fn host_is_not_a_guest() {
        assert!(guests_from_text("Chat with Jane Doe", "", "Jane Doe").is_empty());
    }

    #[test]
    fn page_must_match_the_name() {
        let v: Value = serde_json::json!({"query":{"pages":{
            "1":{"index":1,"title":"Jane Doe (disambiguation)","extract":"Jane Doe may refer to many different people in many places."},
            "2":{"index":2,"title":"Jane Doeson","extract":"A completely different person who is long enough to pass the length check."},
            "3":{"index":3,"title":"Jane Doe (author)","description":"American author","extract":"Jane Doe is an American author known for several widely read novels.","thumbnail":{"source":"https://x/y.jpg"}}
        }}});
        let bio = pick_page("Jane Doe", &v).expect("match");
        assert_eq!(bio.article, "Jane Doe (author)");
        assert_eq!(bio.photo.as_deref(), Some("https://x/y.jpg"));
        assert!(pick_page("John Roe", &v).is_none());
    }
}
