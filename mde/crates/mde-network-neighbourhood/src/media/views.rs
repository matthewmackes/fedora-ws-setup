//! Pure builders that arrange a flat list of media entries into the virtual
//! folder views shown in the mount: By Artist / By Album / By Genre /
//! Recently Added. No network or FUSE here, so it is unit-testable.

use std::collections::BTreeMap;

/// One playable item from a media server.
#[derive(Debug, Clone, Default)]
pub struct Entry {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    /// Track number within the album (0 = unknown).
    pub track: u32,
    /// Unix seconds the server added it (0 = unknown).
    pub added: u64,
    pub size: u64,
    pub url: String,
    /// File extension without the dot ("mp3", "mkv").
    pub ext: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewFile {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ViewDir {
    pub name: String,
    pub dirs: Vec<ViewDir>,
    pub files: Vec<ViewFile>,
}

/// Keep names safe as one path component.
pub fn clean(s: &str, fallback: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| if c == '/' || c == '\0' || c.is_control() { '_' } else { c })
        .collect();
    let t = t.trim().trim_matches('.').to_string();
    if t.is_empty() {
        fallback.to_string()
    } else {
        t
    }
}

fn file_name(e: &Entry) -> String {
    let base = if e.track > 0 {
        format!("{:02} - {}", e.track, clean(&e.title, "Untitled"))
    } else {
        clean(&e.title, "Untitled")
    };
    if e.ext.is_empty() {
        base
    } else {
        format!("{base}.{}", e.ext)
    }
}

fn file(e: &Entry) -> ViewFile {
    ViewFile {
        name: file_name(e),
        url: e.url.clone(),
        size: e.size,
    }
}

fn album_label(e: &Entry) -> String {
    let artist = clean(&e.artist, "Unknown Artist");
    let album = clean(&e.album, "Unknown Album");
    format!("{artist} - {album}")
}

/// Group entries by key into sub-folders, sorted by key; files sorted by name.
fn group<F: Fn(&Entry) -> String>(entries: &[&Entry], key: F) -> Vec<ViewDir> {
    let mut map: BTreeMap<String, Vec<&Entry>> = BTreeMap::new();
    for e in entries {
        map.entry(key(e)).or_default().push(e);
    }
    map.into_iter()
        .map(|(name, items)| {
            let mut files: Vec<ViewFile> = items.iter().map(|e| file(e)).collect();
            files.sort_by(|a, b| a.name.cmp(&b.name));
            ViewDir { name, dirs: vec![], files }
        })
        .collect()
}

/// The four top-level views for a music library.
pub fn audio_views(entries: &[Entry]) -> Vec<ViewDir> {
    let all: Vec<&Entry> = entries.iter().collect();

    // By Artist → Artist → Album → tracks
    let mut by_artist: BTreeMap<String, Vec<&Entry>> = BTreeMap::new();
    for e in &all {
        by_artist.entry(clean(&e.artist, "Unknown Artist")).or_default().push(e);
    }
    let by_artist: Vec<ViewDir> = by_artist
        .into_iter()
        .map(|(name, items)| ViewDir {
            name,
            dirs: group(&items, |e| clean(&e.album, "Unknown Album")),
            files: vec![],
        })
        .collect();

    // By Album → "Artist - Album" → tracks
    let by_album = group(&all, album_label);

    // By Genre → Genre → "Artist - Album" → tracks
    let mut by_genre: BTreeMap<String, Vec<&Entry>> = BTreeMap::new();
    for e in &all {
        by_genre.entry(clean(&e.genre, "Unknown Genre")).or_default().push(e);
    }
    let by_genre: Vec<ViewDir> = by_genre
        .into_iter()
        .map(|(name, items)| ViewDir { name, dirs: group(&items, album_label), files: vec![] })
        .collect();

    // Recently Added → newest 50 albums, newest first (prefix keeps the order sorted)
    let mut newest: BTreeMap<String, u64> = BTreeMap::new();
    for e in &all {
        let k = album_label(e);
        let t = newest.entry(k).or_insert(0);
        *t = (*t).max(e.added);
    }
    let mut order: Vec<(String, u64)> = newest.into_iter().collect();
    order.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    order.truncate(50);
    let recent: Vec<ViewDir> = order
        .iter()
        .enumerate()
        .map(|(i, (label, _))| {
            let items: Vec<&Entry> = all.iter().copied().filter(|e| album_label(e) == *label).collect();
            let mut files: Vec<ViewFile> = items.iter().map(|e| file(e)).collect();
            files.sort_by(|a, b| a.name.cmp(&b.name));
            ViewDir { name: format!("{:02} {label}", i + 1), dirs: vec![], files }
        })
        .collect();

    vec![
        ViewDir { name: "By Artist".into(), dirs: by_artist, files: vec![] },
        ViewDir { name: "By Album".into(), dirs: by_album, files: vec![] },
        ViewDir { name: "By Genre".into(), dirs: by_genre, files: vec![] },
        ViewDir { name: "Recently Added".into(), dirs: recent, files: vec![] },
    ]
}

/// The same four-view idea for video: By Title, By Genre, By Year is not
/// available generically, so: All (A–Z), By Genre, Recently Added.
pub fn video_views(entries: &[Entry]) -> Vec<ViewDir> {
    let all: Vec<&Entry> = entries.iter().collect();
    let mut a_z: Vec<ViewFile> = all.iter().map(|e| ViewFile { name: file_name_plain(e), ..file(e) }).collect();
    a_z.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let by_genre: Vec<ViewDir> = {
        let mut map: BTreeMap<String, Vec<&Entry>> = BTreeMap::new();
        for e in &all {
            map.entry(clean(&e.genre, "Unknown Genre")).or_default().push(e);
        }
        map.into_iter()
            .map(|(name, items)| {
                let mut files: Vec<ViewFile> =
                    items.iter().map(|e| ViewFile { name: file_name_plain(e), ..file(e) }).collect();
                files.sort_by(|a, b| a.name.cmp(&b.name));
                ViewDir { name, dirs: vec![], files }
            })
            .collect()
    };

    let mut newest: Vec<&Entry> = all.clone();
    newest.sort_by(|a, b| b.added.cmp(&a.added).then(a.title.cmp(&b.title)));
    newest.truncate(50);
    let recent: Vec<ViewFile> = newest
        .iter()
        .enumerate()
        .map(|(i, e)| ViewFile { name: format!("{:02} {}", i + 1, file_name_plain(e)), ..file(e) })
        .collect();

    vec![
        ViewDir { name: "All".into(), dirs: vec![], files: a_z },
        ViewDir { name: "By Genre".into(), dirs: by_genre, files: vec![] },
        ViewDir { name: "Recently Added".into(), dirs: vec![], files: recent },
    ]
}

fn file_name_plain(e: &Entry) -> String {
    let base = clean(&e.title, "Untitled");
    if e.ext.is_empty() {
        base
    } else {
        format!("{base}.{}", e.ext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(title: &str, artist: &str, album: &str, genre: &str, track: u32, added: u64) -> Entry {
        Entry {
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            genre: genre.into(),
            track,
            added,
            size: 10,
            url: format!("http://x/{title}"),
            ext: "mp3".into(),
        }
    }

    fn lib() -> Vec<Entry> {
        vec![
            e("Smooth Criminal", "Alien Ant Farm", "ANThology", "Rock", 1, 300),
            e("Movies", "Alien Ant Farm", "ANThology", "Rock", 2, 300),
            e("Bad", "Michael Jackson", "Bad", "Pop", 1, 100),
            e("Untagged", "", "", "", 0, 0),
        ]
    }

    fn dir<'a>(d: &'a [ViewDir], name: &str) -> &'a ViewDir {
        d.iter().find(|x| x.name == name).unwrap_or_else(|| panic!("no dir {name}"))
    }

    #[test]
    fn four_top_level_views() {
        let v = audio_views(&lib());
        let names: Vec<_> = v.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["By Artist", "By Album", "By Genre", "Recently Added"]);
    }

    #[test]
    fn by_artist_nests_album_then_numbered_tracks() {
        let v = audio_views(&lib());
        let artist = dir(&dir(&v, "By Artist").dirs, "Alien Ant Farm");
        let album = dir(&artist.dirs, "ANThology");
        assert_eq!(album.files[0].name, "01 - Smooth Criminal.mp3");
        assert_eq!(album.files[1].name, "02 - Movies.mp3");
    }

    #[test]
    fn by_album_and_genre() {
        let v = audio_views(&lib());
        assert!(dir(&dir(&v, "By Album").dirs, "Alien Ant Farm - ANThology").files.len() == 2);
        let rock = dir(&dir(&v, "By Genre").dirs, "Rock");
        assert!(dir(&rock.dirs, "Alien Ant Farm - ANThology").files.len() == 2);
    }

    #[test]
    fn missing_tags_get_placeholders() {
        let v = audio_views(&lib());
        assert!(dir(&dir(&v, "By Artist").dirs, "Unknown Artist").dirs.iter().any(|d| d.name == "Unknown Album"));
        assert!(dir(&v, "By Genre").dirs.iter().any(|d| d.name == "Unknown Genre"));
    }

    #[test]
    fn recent_is_newest_first() {
        let v = audio_views(&lib());
        let r = &dir(&v, "Recently Added").dirs;
        assert_eq!(r[0].name, "01 Alien Ant Farm - ANThology");
        assert!(r[1].name.starts_with("02 "));
    }

    #[test]
    fn slashes_are_neutralised() {
        assert_eq!(clean("AC/DC", "x"), "AC_DC");
        assert_eq!(clean("  ", "x"), "x");
    }

    #[test]
    fn video_views_work() {
        let mut v = e("Alien", "", "", "Horror", 0, 5);
        v.ext = "mkv".into();
        let views = video_views(&[v]);
        assert_eq!(views[0].files[0].name, "Alien.mkv");
        assert_eq!(dir(&views, "By Genre").dirs[0].name, "Horror");
        assert!(views[2].files[0].name.starts_with("01 "));
    }
}
