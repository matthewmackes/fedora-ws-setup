//! Hard-merge catalogs across servers + YouTube Music (prefer higher quality).

use crate::model::*;

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn album_key(a: &Album) -> String {
    format!("{}::{}", norm(&a.artist), norm(&a.name))
}

fn track_key(t: &Track) -> String {
    format!(
        "{}::{}::{}",
        norm(&t.artist),
        norm(&t.album),
        norm(&t.title)
    )
}

fn quality_score(t: &Track) -> u64 {
    let br = t.bit_rate.unwrap_or(0) as u64;
    let sr = t.sample_rate.unwrap_or(0) as u64;
    let fmt = t.format.as_deref().unwrap_or("").to_lowercase();
    let fmt_bonus = if fmt.contains("flac") || fmt.contains("alac") {
        500_000
    } else if fmt.contains("opus") || fmt.contains("aac") || fmt.contains("m4a") {
        50_000
    } else if fmt.contains("ytm") || fmt.contains("ytdlp") {
        5_000
    } else {
        0
    };
    // Prefer non-YTM on tie via small local bonus.
    let local_bonus = match t.source {
        SourceKind::YoutubeMusic => 0,
        SourceKind::Server | SourceKind::Local | SourceKind::Merged => 1,
    };
    br.saturating_mul(1000) + sr + fmt_bonus + local_bonus
}

pub fn merge_albums(mut primary: Vec<Album>, secondary: Vec<Album>) -> Vec<Album> {
    for a in secondary {
        if let Some(existing) = primary.iter_mut().find(|p| album_key(p) == album_key(&a)) {
            if !existing.alt_ids.contains(&a.id) {
                existing.alt_ids.push(a.id.clone());
            }
            if existing.cover_url.is_none() {
                existing.cover_url = a.cover_url.clone();
            }
            existing.source = SourceKind::Merged;
        } else {
            primary.push(a);
        }
    }
    primary
}

pub fn merge_tracks(mut primary: Vec<Track>, secondary: Vec<Track>) -> Vec<Track> {
    for t in secondary {
        if let Some(existing) = primary.iter_mut().find(|p| track_key(p) == track_key(&t)) {
            if quality_score(&t) > quality_score(existing) {
                let mut winner = t;
                winner.source = SourceKind::Merged;
                *existing = winner;
            } else {
                existing.source = SourceKind::Merged;
            }
        } else {
            primary.push(t);
        }
    }
    primary
}

pub fn merge_search(a: SearchResults, b: SearchResults) -> SearchResults {
    SearchResults {
        artists: {
            let mut out = a.artists;
            for artist in b.artists {
                if !out.iter().any(|x| norm(&x.name) == norm(&artist.name)) {
                    out.push(artist);
                }
            }
            out
        },
        albums: merge_albums(a.albums, b.albums),
        tracks: merge_tracks(a.tracks, b.tracks),
        playlists: {
            let mut out = a.playlists;
            out.extend(b.playlists);
            out
        },
    }
}

pub fn prefer_track<'a>(a: &'a Track, b: &'a Track) -> &'a Track {
    if quality_score(a) >= quality_score(b) {
        a
    } else {
        b
    }
}
