mod airsonic;
mod jellyfin;
pub mod views;

use crate::config::{read_password, Config};
use crate::place::{Place, Protocol};
use anyhow::{bail, Context, Result};
use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    Request,
};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TTL: Duration = Duration::from_secs(5);

#[derive(Clone)]
enum Node {
    Dir { name: String, children: Vec<u64> },
    File { name: String, url: String, size: u64 },
}

struct MediaFs {
    place: Place,
    /// Jellyfin API token / Airsonic password for authenticated range reads.
    auth_token: String,
    inodes: HashMap<u64, Node>,
    next_ino: u64,
}

impl MediaFs {
    fn build(place: Place) -> Result<Self> {
        let password = read_password(&place).unwrap_or_default();
        let top = match place.protocol {
            Protocol::Video => {
                views::video_views(&jellyfin::list_entries(&place, &password)?)
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
        };
        let mut root_children = Vec::new();
        // Virtual "Open in browser" marker file
        let open_ino = fs.alloc(Node::File {
            name: "Open in Browser.url".into(),
            url: fs.place_url(),
            size: 64,
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
        let (kind, size) = match node {
            Node::Dir { .. } => (FileType::Directory, 0),
            Node::File { size, .. } => (FileType::RegularFile, *size),
        };
        FileAttr {
            ino,
            size,
            blocks: size.div_ceil(512),
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            crtime: UNIX_EPOCH,
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
    if jellyfin_auth && !token.is_empty() {
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
