mod airsonic;
mod jellyfin;
pub mod meta;
pub mod views;

use crate::config::{read_password, Config};
use crate::place::{Place, Protocol};
use anyhow::{bail, Context, Result};
use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    ReplyXattr, Request,
};
use std::sync::Arc;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TTL: Duration = Duration::from_secs(5);

#[derive(Clone)]
enum Node {
    Dir { name: String, children: Vec<u64> },
    File {
        name: String,
        url: String,
        size: u64,
        /// Unix seconds the server added it (0 = unknown); shown as the file's date.
        mtime: u64,
        /// Extended attributes served to file browsers.
        meta: Arc<meta::Meta>,
    },
}

struct MediaFs {
    place: Place,
    /// Jellyfin API token / Airsonic password for authenticated range reads.
    auth_token: String,
    inodes: HashMap<u64, Node>,
    next_ino: u64,
    /// Live Jellyfin session, for details fetched when a file browser asks.
    jf: Option<jellyfin::Conn>,
    /// Cast/crew already fetched, by inode.
    lazy_cache: HashMap<u64, meta::Meta>,
}

impl MediaFs {
    fn build(place: Place) -> Result<Self> {
        let password = read_password(&place).unwrap_or_default();
        let mut jf = None;
        let top = match place.protocol {
            Protocol::Video => {
                let conn = jellyfin::connect(&place, &password, &jellyfin::session_path(&place))?;
                let entries = jellyfin::list_with(&conn)?;
                jf = Some(conn);
                views::video_views(&entries)
            }
            Protocol::Audio => {
                views::audio_views(&airsonic::list_entries(&place, &password)?)
            }
            _ => bail!("not a media place"),
        };

        let mut fs = Self {
            place,
            auth_token: password,
            inodes: HashMap::new(),
            next_ino: 2,
            jf,
            lazy_cache: HashMap::new(),
        };
        let mut root_children = Vec::new();
        // Virtual "Open in browser" marker file
        let open_ino = fs.alloc(Node::File {
            name: "Open in Browser.url".into(),
            url: fs.place_url(),
            size: 64,
            mtime: 0,
            meta: Arc::default(),
        });
        root_children.push(open_ino);
        for dir in &top {
            root_children.push(fs.add_view(dir));
        }

        fs.inodes.insert(
            1,
            Node::Dir {
                name: "/".into(),
                children: root_children,
            },
        );
        Ok(fs)
    }

    /// Turn a view tree into inodes, returning the directory's inode.
    fn add_view(&mut self, dir: &views::ViewDir) -> u64 {
        let mut children = Vec::new();
        for d in &dir.dirs {
            children.push(self.add_view(d));
        }
        for f in &dir.files {
            children.push(self.alloc(Node::File {
                name: sanitize_name(&f.name),
                url: f.url.clone(),
                size: f.size,
                mtime: f.mtime,
                meta: f.meta.clone(),
            }));
        }
        self.alloc(Node::Dir {
            name: sanitize_name(&dir.name),
            children,
        })
    }

    fn place_url(&self) -> String {
        if !self.place.url.is_empty() {
            self.place.url.clone()
        } else {
            let scheme = "http";
            let port = self.place.port.unwrap_or(match self.place.protocol {
                Protocol::Video => 8096,
                Protocol::Audio => 4040,
                _ => 80,
            });
            format!("{scheme}://{}:{port}", self.place.host)
        }
    }

    fn alloc(&mut self, node: Node) -> u64 {
        let ino = self.next_ino;
        self.next_ino += 1;
        self.inodes.insert(ino, node);
        ino
    }

    fn attr(ino: u64, node: &Node) -> FileAttr {
        let (kind, size, stamp) = match node {
            Node::Dir { .. } => (FileType::Directory, 0, 0),
            Node::File { size, mtime, .. } => (FileType::RegularFile, *size, *mtime),
        };
        // Use the server's date-added as the file date when we have it.
        let when = if stamp > 0 {
            UNIX_EPOCH + Duration::from_secs(stamp)
        } else {
            SystemTime::now()
        };
        FileAttr {
            ino,
            size,
            blocks: size.div_ceil(512),
            atime: when,
            mtime: when,
            ctime: when,
            crtime: if stamp > 0 { when } else { UNIX_EPOCH },
            kind,
            perm: if kind == FileType::Directory {
                0o755
            } else {
                0o644
            },
            nlink: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
            blksize: 512,
            flags: 0,
        }
    }

    /// Value of one extended attribute, fetching expensive ones on first use.
    fn xattr_value(&mut self, ino: u64, name: &str) -> Option<String> {
        let (meta, id) = match self.inodes.get(&ino)? {
            Node::File { meta, .. } => (meta.clone(), meta.get("user.media.id").cloned()),
            Node::Dir { .. } => return None,
        };
        if let Some(v) = meta.get(name) {
            return Some(v.clone());
        }
        if !meta::LAZY.contains(&name) {
            return None;
        }
        if !self.lazy_cache.contains_key(&ino) {
            let fetched = match (&self.jf, id) {
                (Some(conn), Some(id)) => jellyfin::person_meta(conn, &id).unwrap_or_default(),
                _ => meta::Meta::new(),
            };
            self.lazy_cache.insert(ino, fetched);
        }
        self.lazy_cache.get(&ino)?.get(name).cloned()
    }

    /// Every attribute name this file can answer for (eager ones plus the lazy ones).
    fn xattr_names(&self, ino: u64) -> Vec<String> {
        match self.inodes.get(&ino) {
            Some(Node::File { meta, .. }) => {
                let mut v: Vec<String> = meta.keys().cloned().collect();
                if self.jf.is_some() && meta.contains_key("user.media.id") {
                    for l in meta::LAZY {
                        if !v.iter().any(|k| k == l) {
                            v.push(l.to_string());
                        }
                    }
                }
                v
            }
            _ => Vec::new(),
        }
    }
}

impl Filesystem for MediaFs {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name = name.to_string_lossy();
        let Some(Node::Dir { children, .. }) = self.inodes.get(&parent).cloned() else {
            reply.error(libc::ENOENT);
            return;
        };
        for ino in children {
            if let Some(node) = self.inodes.get(&ino) {
                let n = match node {
                    Node::Dir { name, .. } | Node::File { name, .. } => name.as_str(),
                };
                if n == name {
                    reply.entry(&TTL, &Self::attr(ino, node), 0);
                    return;
                }
            }
        }
        reply.error(libc::ENOENT);
    }

    fn getxattr(&mut self, _req: &Request, ino: u64, name: &OsStr, size: u32, reply: ReplyXattr) {
        match self.xattr_value(ino, &name.to_string_lossy()) {
            None => reply.error(libc::ENODATA),
            Some(v) => match meta::xattr_reply(v.as_bytes(), size) {
                meta::XattrReply::Size(n) => reply.size(n),
                meta::XattrReply::Data(d) => reply.data(&d),
                meta::XattrReply::TooSmall => reply.error(libc::ERANGE),
            },
        }
    }

    fn listxattr(&mut self, _req: &Request, ino: u64, size: u32, reply: ReplyXattr) {
        let names = self.xattr_names(ino);
        let wire = meta::encode_names(names.iter().map(String::as_str));
        match meta::xattr_reply(&wire, size) {
            meta::XattrReply::Size(n) => reply.size(n),
            meta::XattrReply::Data(d) => reply.data(&d),
            meta::XattrReply::TooSmall => reply.error(libc::ERANGE),
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        match self.inodes.get(&ino) {
            Some(node) => reply.attr(&TTL, &Self::attr(ino, node)),
            None => reply.error(libc::ENOENT),
        }
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        let Some(Node::File { name, url, .. }) = self.inodes.get(&ino).cloned() else {
            reply.error(libc::EISDIR);
            return;
        };
        if name.ends_with(".url") || url.contains("#/") {
            let body = format!("[InternetShortcut]\nURL={url}\n");
            let bytes = body.as_bytes();
            let start = offset as usize;
            if start >= bytes.len() {
                reply.data(&[]);
                return;
            }
            let end = (start + size as usize).min(bytes.len());
            reply.data(&bytes[start..end]);
            return;
        }
        if url.starts_with("http://") || url.starts_with("https://") {
            let token = self.auth_token.clone();
            let is_jellyfin = matches!(self.place.protocol, Protocol::Video);
            match fetch_range(&url, offset as u64, size as u64, &token, is_jellyfin) {
                Ok(data) => reply.data(&data),
                Err(_) => reply.error(libc::EIO),
            }
        } else {
            reply.error(libc::EIO);
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let Some(Node::Dir { children, .. }) = self.inodes.get(&ino).cloned() else {
            reply.error(libc::ENOTDIR);
            return;
        };
        let mut entries: Vec<(u64, FileType, String)> = vec![
            (ino, FileType::Directory, ".".into()),
            (1, FileType::Directory, "..".into()),
        ];
        for child in children {
            if let Some(node) = self.inodes.get(&child) {
                let (kind, name) = match node {
                    Node::Dir { name, .. } => (FileType::Directory, name.clone()),
                    Node::File { name, .. } => (FileType::RegularFile, name.clone()),
                };
                entries.push((child, kind, name));
            }
        }
        for (i, (cino, kind, name)) in entries.into_iter().enumerate().skip(offset as usize) {
            if reply.add(cino, (i + 1) as i64, kind, name) {
                break;
            }
        }
        reply.ok();
    }
}

fn fetch_range(
    url: &str,
    offset: u64,
    size: u64,
    token: &str,
    jellyfin_auth: bool,
) -> Result<Vec<u8>> {
    let end = offset + size.saturating_sub(1);
    let mut req = ureq::get(url).set("Range", &format!("bytes={offset}-{end}"));
    // Jellyfin URLs from list_entries already carry `api_key=`; don't override it.
    if jellyfin_auth && !token.is_empty() && !url.contains("api_key=") {
        req = req
            .set("X-Emby-Token", token)
            .set("Authorization", &format!("MediaBrowser Token=\"{token}\""));
    }
    let resp = req.call().context("http get")?;
    let mut data = Vec::new();
    resp.into_reader()
        .take(size)
        .read_to_end(&mut data)
        .context("read body")?;
    Ok(data)
}

fn sanitize_name(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| match c {
            '/' | '\0' => '_',
            c => c,
        })
        .collect();
    if out.is_empty() {
        out = "item".into();
    }
    out
}

/// Verify the username/password against the media server without listing anything.
pub fn check_login(place: &Place, password: &str) -> Result<String> {
    match place.protocol {
        Protocol::Video => jellyfin::check_login(place, password),
        Protocol::Audio => airsonic::check_login(place, password),
        _ => bail!("not a media place"),
    }
}

/// Mount a Video/Audio place with FUSE (blocking).
pub fn mount_blocking(place_id: &str, mountpoint: &Path) -> Result<()> {
    let cfg = Config::load()?;
    let place = cfg
        .find(place_id)
        .cloned()
        .with_context(|| format!("unknown place {place_id}"))?;
    let fs = MediaFs::build(place)?;
    let opts = vec![
        MountOption::FSName("neighbourhood-mediafs".into()),
        MountOption::AllowOther,
        MountOption::AutoUnmount,
        MountOption::DefaultPermissions,
    ];
    fuser::mount2(fs, mountpoint, &opts).context("fuse mount")?;
    Ok(())
}

pub fn open_in_browser(place: &Place) -> Result<()> {
    let url = if !place.url.is_empty() {
        place.url.clone()
    } else {
        let port = place.port.unwrap_or(match place.protocol {
            Protocol::Video => 8096,
            Protocol::Audio => 4040,
            _ => 80,
        });
        format!("http://{}:{port}", place.host)
    };
    let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    Ok(())
}

#[cfg(test)]
mod live {
    //! Manual checks against a real server: `NN_LIVE_HOST=172.20.0.2 cargo test live -- --ignored --nocapture`.
    use crate::place::{Place, Protocol};

    fn host() -> String {
        std::env::var("NN_LIVE_HOST").unwrap_or_default()
    }

    #[test]
    #[ignore]
    fn airsonic_rejects_bad_login_with_a_clear_message() {
        let mut p = Place::new("a", Protocol::Audio, &host());
        p.username = "nn-probe".into();
        let err = super::airsonic::list_entries(&p, "x").unwrap_err().to_string();
        println!("airsonic: {err}");
        assert!(err.contains("Wrong username or password"), "{err}");
    }

    #[test]
    #[ignore]
    fn jellyfin_rejects_bad_login() {
        let mut p = Place::new("j", Protocol::Video, &host());
        p.username = "nn-probe".into();
        let err = format!("{:#}", super::jellyfin::list_entries(&p, "x").unwrap_err());
        println!("jellyfin: {err}");
        assert!(err.contains("401") || err.to_lowercase().contains("login"), "{err}");
    }

    #[test]
    #[ignore]
    fn airsonic_lists_real_library() {
        let mut p = Place::new("a", Protocol::Audio, &host());
        p.username = std::env::var("NN_LIVE_USER").unwrap_or_default();
        let pw = std::env::var("NN_LIVE_PASS").unwrap_or_default();
        let entries = super::airsonic::list_entries(&p, &pw).expect("list");
        println!("songs: {}", entries.len());
        let views = crate::media::views::audio_views(&entries);
        for v in &views {
            println!("  {}: {} entries; first: {:?}", v.name, v.dirs.len(), v.dirs.first().map(|d| d.name.clone()));
        }
        let artist = views[0].dirs.iter().find(|a| a.name.to_lowercase().contains("alien")).cloned();
        println!("alien ant farm: {:?}", artist.map(|a| a.dirs.iter().map(|d| d.name.clone()).collect::<Vec<_>>()));
        assert!(!entries.is_empty());
    }

    #[test]
    #[ignore]
    fn jellyfin_lists_real_library() {
        let mut p = Place::new("j", Protocol::Video, &host());
        p.username = std::env::var("NN_LIVE_USER").unwrap_or_default();
        let pw = std::env::var("NN_LIVE_PASS").unwrap_or_default();
        let entries = super::jellyfin::list_entries(&p, &pw).expect("list");
        println!("items: {}", entries.len());
        let views = crate::media::views::video_views(&entries);
        for v in &views {
            println!("  {}: {} dirs, {} files", v.name, v.dirs.len(), v.files.len());
        }
        println!("first: {:?}", views[0].files.first().map(|f| (f.name.clone(), f.size)));
        assert!(!entries.is_empty());
    }
}
