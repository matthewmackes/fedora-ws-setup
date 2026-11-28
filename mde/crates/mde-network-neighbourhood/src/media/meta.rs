//! Turn a media server's description of one item into extended attributes that
//! file browsers can read (`getfattr`, Dolphin, ...). Pure functions: no network,
//! no FUSE, so they are unit-tested.
//!
//! Names follow two conventions:
//! * `user.media.*`: every detail the server gives us.
//! * `user.xdg.comment|tags|rating|origin.url`: the freedesktop attributes KDE
//!   and other file managers already display and index.
//!
//! Nothing here may contain a password or access token.

use serde_json::Value;
use std::collections::BTreeMap;

pub type Meta = BTreeMap<String, String>;

/// Attributes fetched on demand because they are expensive to list up front.
pub const LAZY: [&str; 3] = ["user.media.cast", "user.media.director", "user.media.writers"];

const M: &str = "user.media.";

fn put(m: &mut Meta, key: &str, value: impl ToString) {
    let v = value.to_string();
    if !v.trim().is_empty() {
        m.insert(format!("{M}{key}"), v);
    }
}

fn put_raw(m: &mut Meta, full_key: &str, value: impl ToString) {
    let v = value.to_string();
    if !v.trim().is_empty() {
        m.insert(full_key.to_string(), v);
    }
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn n(v: &Value, k: &str) -> Option<u64> {
    v.get(k).and_then(|x| x.as_u64())
}

fn f(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(|x| x.as_f64())
}

/// 66 -> "1:06", 3725 -> "1:02:05".
pub fn fmt_duration(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// "2024-05-06T07:08:09Z" -> "2024-05-06".
fn date_part(iso: &str) -> String {
    iso.split('T').next().unwrap_or("").to_string()
}

pub fn audio_meta(song: &Value) -> Meta {
    let mut m = Meta::new();
    put(&mut m, "type", "audio");
    put(&mut m, "title", s(song, "title"));
    put(&mut m, "artist", s(song, "artist"));
    put(&mut m, "album", s(song, "album"));
    put(&mut m, "genre", s(song, "genre"));
    if let Some(y) = n(song, "year").filter(|y| *y > 0) {
        put(&mut m, "year", y);
    }
    if let Some(t) = n(song, "track").filter(|t| *t > 0) {
        put(&mut m, "track", t);
    }
    if let Some(d) = n(song, "discNumber").filter(|d| *d > 0) {
        put(&mut m, "disc", d);
    }
    let duration = n(song, "duration");
    if let Some(d) = duration {
        put(&mut m, "duration_seconds", d);
        put(&mut m, "duration", fmt_duration(d));
    }
    let bitrate = n(song, "bitRate");
    if let Some(b) = bitrate {
        put(&mut m, "bitrate_kbps", b);
    }
    if let Some(v) = n(song, "samplingRate") {
        put(&mut m, "sample_rate_hz", v);
    }
    if let Some(v) = n(song, "channelCount") {
        put(&mut m, "channels", v);
    }
    if let Some(v) = n(song, "bitDepth") {
        put(&mut m, "bit_depth", v);
    }
    put(&mut m, "format", s(song, "suffix"));
    put(&mut m, "content_type", s(song, "contentType"));
    if let Some(v) = n(song, "size") {
        put(&mut m, "size_bytes", v);
    }
    if let Some(v) = n(song, "playCount") {
        put(&mut m, "play_count", v);
    }
    let rating = n(song, "userRating");
    if let Some(r) = rating {
        put(&mut m, "rating", r);
    }
    put(&mut m, "added", s(song, "created"));
    put(&mut m, "server_path", s(song, "path"));
    put(&mut m, "id", s(song, "id"));
    put(&mut m, "album_id", s(song, "albumId"));
    put(&mut m, "artist_id", s(song, "artistId"));
    put(&mut m, "cover_art_id", s(song, "coverArt"));

    // freedesktop attributes
    let mut line = String::new();
    let artist = s(song, "artist");
    let album = s(song, "album");
    if !artist.is_empty() {
        line.push_str(&artist);
    }
    if !album.is_empty() {
        if !line.is_empty() {
            line.push_str(" — ");
        }
        line.push_str(&album);
        if let Some(y) = n(song, "year").filter(|y| *y > 0) {
            line.push_str(&format!(" ({y})"));
        }
    }
    let mut tech = Vec::new();
    if let Some(d) = duration {
        tech.push(fmt_duration(d));
    }
    let fmt = s(song, "suffix").to_uppercase();
    match (fmt.is_empty(), bitrate) {
        (false, Some(b)) => tech.push(format!("{fmt} {b} kbps")),
        (false, None) => tech.push(fmt),
        (true, Some(b)) => tech.push(format!("{b} kbps")),
        _ => {}
    }
    if !tech.is_empty() {
        if !line.is_empty() {
            line.push_str(" · ");
        }
        line.push_str(&tech.join(" · "));
    }
    put_raw(&mut m, "user.xdg.comment", line);
    put_raw(&mut m, "user.xdg.tags", s(song, "genre"));
    if let Some(r) = rating.filter(|r| (1..=5).contains(r)) {
        put_raw(&mut m, "user.xdg.rating", r * 2); // KDE scale is 0-10
    }
    m
}

fn join_names(v: Option<&Value>, key: &str) -> String {
    v.and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.get(key).and_then(|n| n.as_str()))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

fn unique_langs(streams: &[&Value]) -> String {
    let mut seen: Vec<String> = Vec::new();
    for st in streams {
        let l = s(st, "Language");
        if !l.is_empty() && !seen.contains(&l) {
            seen.push(l);
        }
    }
    seen.join(", ")
}

/// `web_base` is the server's public address (no credentials), for the origin URL.
pub fn video_meta(item: &Value, web_base: &str) -> Meta {
    let mut m = Meta::new();
    let kind = s(item, "Type").to_lowercase();
    put(&mut m, "type", if kind.is_empty() { "video".into() } else { kind });
    put(&mut m, "title", s(item, "Name"));
    put(&mut m, "original_title", s(item, "OriginalTitle"));
    if let Some(y) = n(item, "ProductionYear") {
        put(&mut m, "year", y);
    }
    put(&mut m, "premiere_date", date_part(&s(item, "PremiereDate")));
    let overview = s(item, "Overview");
    put(&mut m, "overview", &overview);
    let taglines = item
        .get("Taglines")
        .and_then(|t| t.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" / "))
        .unwrap_or_default();
    put(&mut m, "tagline", taglines);
    let genres = item
        .get("Genres")
        .and_then(|g| g.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", "))
        .unwrap_or_default();
    put(&mut m, "genres", &genres);
    put(&mut m, "studios", join_names(item.get("Studios"), "Name"));
    put(&mut m, "official_rating", s(item, "OfficialRating"));
    let community = f(item, "CommunityRating");
    if let Some(r) = community {
        put(&mut m, "community_rating", format!("{r:.1}"));
    }
    if let Some(r) = f(item, "CriticRating") {
        put(&mut m, "critic_rating", format!("{r:.0}"));
    }
    let runtime = n(item, "RunTimeTicks").map(|t| t / 10_000_000);
    if let Some(r) = runtime {
        put(&mut m, "runtime_seconds", r);
        put(&mut m, "duration", fmt_duration(r));
    }
    if let Some(ids) = item.get("ProviderIds").and_then(|p| p.as_object()) {
        for (k, v) in ids {
            if let Some(v) = v.as_str() {
                put(&mut m, &format!("id.{}", k.to_lowercase()), v);
            }
        }
    }
    // Episodes
    put(&mut m, "series", s(item, "SeriesName"));
    put(&mut m, "season", s(item, "SeasonName"));
    if let Some(v) = n(item, "ParentIndexNumber") {
        put(&mut m, "season_number", v);
    }
    if let Some(v) = n(item, "IndexNumber") {
        put(&mut m, "episode_number", v);
    }

    put(&mut m, "added", s(item, "DateCreated"));
    put(&mut m, "server_path", s(item, "Path"));
    let id = s(item, "Id");
    put(&mut m, "id", &id);

    // Container and streams come from the first media source.
    let src = item.pointer("/MediaSources/0");
    let mut res = String::new();
    let mut vcodec = String::new();
    if let Some(src) = src {
        put(&mut m, "container", s(src, "Container"));
        if let Some(b) = n(src, "Bitrate") {
            put(&mut m, "bitrate_kbps", b / 1000);
        }
        if let Some(sz) = n(src, "Size") {
            put(&mut m, "size_bytes", sz);
        }
        let streams: Vec<&Value> = src
            .get("MediaStreams")
            .and_then(|s| s.as_array())
            .map(|a| a.iter().collect())
            .unwrap_or_default();
        let of = |t: &str| -> Vec<&Value> {
            streams.iter().copied().filter(|s2| s2.get("Type").and_then(|x| x.as_str()) == Some(t)).collect()
        };
        if let Some(v) = of("Video").first() {
            vcodec = s(v, "Codec");
            put(&mut m, "video_codec", &vcodec);
            if let (Some(w), Some(h)) = (n(v, "Width"), n(v, "Height")) {
                put(&mut m, "width", w);
                put(&mut m, "height", h);
                res = format!("{w}x{h}");
                put(&mut m, "resolution", &res);
            }
            if let Some(fr) = f(v, "RealFrameRate").or_else(|| f(v, "AverageFrameRate")) {
                put(&mut m, "frame_rate", format!("{fr:.3}").trim_end_matches('0').trim_end_matches('.'));
            }
            if let Some(b) = n(v, "BitRate") {
                put(&mut m, "video_bitrate_kbps", b / 1000);
            }
            if let Some(d) = n(v, "BitDepth") {
                put(&mut m, "video_bit_depth", d);
            }
            put(&mut m, "dynamic_range", s(v, "VideoRange"));
        }
        let audio = of("Audio");
        if let Some(a) = audio.first() {
            put(&mut m, "audio_codec", s(a, "Codec"));
            if let Some(c) = n(a, "Channels") {
                put(&mut m, "audio_channels", c);
            }
        }
        put(&mut m, "audio_streams", audio.len());
        put(&mut m, "audio_languages", unique_langs(&audio));
        let subs = of("Subtitle");
        put(&mut m, "subtitle_streams", subs.len());
        put(&mut m, "subtitle_languages", unique_langs(&subs));
    }

    // freedesktop attributes
    let comment = if !overview.trim().is_empty() {
        overview.trim().to_string()
    } else {
        let mut parts = Vec::new();
        if !res.is_empty() {
            parts.push(res.clone());
        }
        if !vcodec.is_empty() {
            parts.push(vcodec.to_uppercase());
        }
        parts.join(" · ")
    };
    put_raw(&mut m, "user.xdg.comment", comment);
    put_raw(&mut m, "user.xdg.tags", &genres);
    if let Some(r) = community {
        put_raw(&mut m, "user.xdg.rating", r.round() as u64); // already 0-10
    }
    if !id.is_empty() && !web_base.is_empty() {
        put_raw(&mut m, "user.xdg.origin.url", format!("{web_base}/web/#/details?id={id}"));
    }
    m
}

/// "Name as Role; Name2" for one kind of credit (`Actor`, `Director`, `Writer`).
fn credits(item: &Value, kind: &str, max: usize) -> String {
    item.get("People")
        .and_then(|p| p.as_array())
        .map(|a| {
            a.iter()
                .filter(|p| p.get("Type").and_then(|t| t.as_str()) == Some(kind))
                .take(max)
                .filter_map(|p| {
                    let name = p.get("Name")?.as_str()?;
                    match p.get("Role").and_then(|r| r.as_str()).filter(|r| !r.is_empty()) {
                        Some(role) => Some(format!("{name} as {role}")),
                        None => Some(name.to_string()),
                    }
                })
                .collect::<Vec<_>>()
                .join("; ")
        })
        .unwrap_or_default()
}

/// The lazily-fetched credits for one video.
pub fn people_meta(item: &Value) -> Meta {
    let mut m = Meta::new();
    put_raw(&mut m, "user.media.cast", credits(item, "Actor", 40));
    put_raw(&mut m, "user.media.director", credits(item, "Director", 10));
    put_raw(&mut m, "user.media.writers", credits(item, "Writer", 10));
    m
}

/// How an xattr read must be answered (the `size` argument protocol).
#[derive(Debug, PartialEq, Eq)]
pub enum XattrReply {
    /// Caller asked how big the value is.
    Size(u32),
    Data(Vec<u8>),
    /// Caller's buffer is too small.
    TooSmall,
}

pub fn xattr_reply(value: &[u8], size: u32) -> XattrReply {
    if size == 0 {
        XattrReply::Size(value.len() as u32)
    } else if (size as usize) < value.len() {
        XattrReply::TooSmall
    } else {
        XattrReply::Data(value.to_vec())
    }
}

/// `listxattr` wire format: each name followed by a NUL.
pub fn encode_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<u8> {
    let mut out = Vec::new();
    for n in names {
        out.extend_from_slice(n.as_bytes());
        out.push(0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn song() -> Value {
        json!({"id":"9","title":"Blood Moon Prelude","album":"The Speed of Metal","artist":"Warrant",
               "track":1,"year":2025,"genre":"Speed Metal","coverArt":"23426","size":10152176,
               "contentType":"audio/flac","suffix":"flac","duration":66,"bitRate":1217,
               "path":"Warrant/The Speed of Metal (2025)/01.flac","playCount":22,"discNumber":1,
               "created":"2026-01-27T02:00:27.000Z","albumId":"5","artistId":"3","userRating":4})
    }

    #[test]
    fn audio_has_every_field_the_server_gave() {
        let m = audio_meta(&song());
        let g = |k: &str| m.get(k).map(String::as_str);
        assert_eq!(g("user.media.title"), Some("Blood Moon Prelude"));
        assert_eq!(g("user.media.artist"), Some("Warrant"));
        assert_eq!(g("user.media.year"), Some("2025"));
        assert_eq!(g("user.media.duration"), Some("1:06"));
        assert_eq!(g("user.media.duration_seconds"), Some("66"));
        assert_eq!(g("user.media.bitrate_kbps"), Some("1217"));
        assert_eq!(g("user.media.format"), Some("flac"));
        assert_eq!(g("user.media.play_count"), Some("22"));
        assert_eq!(g("user.media.disc"), Some("1"));
        assert_eq!(g("user.media.size_bytes"), Some("10152176"));
        assert_eq!(g("user.media.cover_art_id"), Some("23426"));
        assert_eq!(g("user.xdg.tags"), Some("Speed Metal"));
        assert_eq!(g("user.xdg.rating"), Some("8"));
        assert_eq!(
            g("user.xdg.comment"),
            Some("Warrant — The Speed of Metal (2025) · 1:06 · FLAC 1217 kbps")
        );
    }

    #[test]
    fn missing_fields_are_omitted_not_blank() {
        let m = audio_meta(&json!({"id":"1","title":"X"}));
        assert!(m.contains_key("user.media.title"));
        assert!(!m.contains_key("user.media.artist"));
        assert!(!m.contains_key("user.media.year"));
        assert!(!m.contains_key("user.xdg.rating"));
        assert!(m.values().all(|v| !v.trim().is_empty()));
    }

    fn movie() -> Value {
        json!({"Id":"abc","Name":"Central Intelligence","Type":"Movie","ProductionYear":2016,
               "OfficialRating":"PG-13","CommunityRating":6.3,"CriticRating":71.0,
               "RunTimeTicks":69868660430u64,"PremiereDate":"2016-06-16T00:00:00.0000000Z",
               "Genres":["Action","Comedy"],"Overview":"A mild-mannered accountant.",
               "Studios":[{"Name":"New Line Cinema"},{"Name":"Bluegrass Films"}],
               "ProviderIds":{"Tmdb":"302699","Imdb":"tt1489889"},"DateCreated":"2024-05-06T07:08:09Z",
               "Path":"/media/movies/CI.mp4","Taglines":["Saving the world."],
               "MediaSources":[{"Container":"mp4","Bitrate":2000000,"Size":1541563889,
                 "MediaStreams":[
                   {"Type":"Video","Codec":"mpeg4","Width":720,"Height":304,"RealFrameRate":23.976,"BitDepth":8},
                   {"Type":"Audio","Codec":"ac3","Channels":6,"Language":"eng"},
                   {"Type":"Audio","Codec":"aac","Channels":2,"Language":"fra"},
                   {"Type":"Subtitle","Codec":"srt","Language":"eng"}]}]})
    }

    #[test]
    fn video_has_technical_and_descriptive_details() {
        let m = video_meta(&movie(), "http://h:8096");
        let g = |k: &str| m.get(k).map(String::as_str);
        assert_eq!(g("user.media.type"), Some("movie"));
        assert_eq!(g("user.media.resolution"), Some("720x304"));
        assert_eq!(g("user.media.video_codec"), Some("mpeg4"));
        assert_eq!(g("user.media.audio_codec"), Some("ac3"));
        assert_eq!(g("user.media.audio_channels"), Some("6"));
        assert_eq!(g("user.media.audio_languages"), Some("eng, fra"));
        assert_eq!(g("user.media.audio_streams"), Some("2"));
        assert_eq!(g("user.media.subtitle_languages"), Some("eng"));
        assert_eq!(g("user.media.frame_rate"), Some("23.976"));
        assert_eq!(g("user.media.runtime_seconds"), Some("6986"));
        assert_eq!(g("user.media.duration"), Some("1:56:26"));
        assert_eq!(g("user.media.premiere_date"), Some("2016-06-16"));
        assert_eq!(g("user.media.genres"), Some("Action, Comedy"));
        assert_eq!(g("user.media.studios"), Some("New Line Cinema, Bluegrass Films"));
        assert_eq!(g("user.media.id.imdb"), Some("tt1489889"));
        assert_eq!(g("user.media.official_rating"), Some("PG-13"));
        assert_eq!(g("user.media.community_rating"), Some("6.3"));
        assert_eq!(g("user.media.bitrate_kbps"), Some("2000"));
        assert_eq!(g("user.xdg.rating"), Some("6"));
        assert_eq!(g("user.xdg.comment"), Some("A mild-mannered accountant."));
        assert_eq!(g("user.xdg.origin.url"), Some("http://h:8096/web/#/details?id=abc"));
    }

    #[test]
    fn no_secret_ever_lands_in_an_attribute() {
        let mut item = movie();
        item["Path"] = json!("/media/x.mp4");
        for m in [video_meta(&item, "http://h:8096"), audio_meta(&song())] {
            for (k, v) in &m {
                assert!(!v.contains("api_key") && !v.contains("token") && !v.contains("password"), "{k}={v}");
            }
        }
    }

    #[test]
    fn episode_details() {
        let ep = json!({"Id":"e1","Name":"Pilot","Type":"Episode","SeriesName":"Show","SeasonName":"Season 1",
                        "ParentIndexNumber":1,"IndexNumber":2});
        let m = video_meta(&ep, "");
        assert_eq!(m["user.media.series"], "Show");
        assert_eq!(m["user.media.season_number"], "1");
        assert_eq!(m["user.media.episode_number"], "2");
        assert!(!m.contains_key("user.xdg.origin.url"), "no URL without a server address");
    }

    #[test]
    fn credits_are_split_by_role() {
        let item = json!({"People":[
            {"Name":"Dwayne Johnson","Type":"Actor","Role":"Bob"},
            {"Name":"Kevin Hart","Type":"Actor"},
            {"Name":"Rawson Marshall Thurber","Type":"Director"}]});
        let m = people_meta(&item);
        assert_eq!(m["user.media.cast"], "Dwayne Johnson as Bob; Kevin Hart");
        assert_eq!(m["user.media.director"], "Rawson Marshall Thurber");
        assert!(!m.contains_key("user.media.writers"));
    }

    #[test]
    fn xattr_size_protocol() {
        assert_eq!(xattr_reply(b"abc", 0), XattrReply::Size(3));
        assert_eq!(xattr_reply(b"abc", 2), XattrReply::TooSmall);
        assert_eq!(xattr_reply(b"abc", 3), XattrReply::Data(b"abc".to_vec()));
        assert_eq!(encode_names(["a", "bc"]), b"a\0bc\0");
    }

    #[test]
    fn durations() {
        assert_eq!(fmt_duration(66), "1:06");
        assert_eq!(fmt_duration(3725), "1:02:05");
        assert_eq!(fmt_duration(0), "0:00");
    }
}
