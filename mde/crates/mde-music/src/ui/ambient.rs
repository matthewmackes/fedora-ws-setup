//! Global full-bleed host controller (W676, W677, W650–W655).
//!
//! One `Bleed` sits behind every page. Pages (or the player) call `set_context`
//! with an artist / album cover; images are resolved and quality-gated on a
//! worker thread, then handed to the backdrop on the main thread.

use super::bleed::{Bleed, Mode};
use crate::artimg::{self, Kind, Use};
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

thread_local! {
    static AMBIENT: RefCell<Option<Bleed>> = const { RefCell::new(None) };
    static PLAYING: RefCell<Option<Ctx>> = const { RefCell::new(None) };
    static LAST: RefCell<Option<Ctx>> = const { RefCell::new(None) };
}

static GEN: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ctx {
    pub artist: String,
    pub album: String,
    pub cover_url: Option<String>,
}

pub fn install(b: Bleed) {
    AMBIENT.with(|a| *a.borrow_mut() = Some(b));
}

pub fn handle() -> Option<Bleed> {
    AMBIENT.with(|a| a.borrow().clone())
}

pub fn set_pulse(level: f64) {
    if let Some(b) = handle() {
        b.set_pulse(level);
    }
}

pub fn set_scroll(px: f64) {
    if let Some(b) = handle() {
        b.set_scroll(px);
    }
}

/// Remember what is playing; non-art pages fall back to it.
pub fn set_playing(ctx: Ctx) {
    PLAYING.with(|p| *p.borrow_mut() = Some(ctx));
}

/// Show `ctx` full-bleed. `Sharp` for hero pages, `Ambient` for dense ones.
pub fn set_context(mode: Mode, ctx: Ctx) {
    set_context_ex(mode, ctx, None, None);
}

/// Same, with a MusicBrainz id / Wikipedia title for exact artist matching.
pub fn set_context_ex(mode: Mode, ctx: Ctx, mbid: Option<String>, wiki: Option<String>) {
    let Some(b) = handle() else { return };
    b.set_mode(mode);
    LAST.with(|l| *l.borrow_mut() = Some(ctx.clone()));
    let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        let paths = resolve_paths(&ctx, mode, mbid.as_deref(), wiki.as_deref());
        if GEN.load(Ordering::SeqCst) != gen || paths.is_empty() {
            return;
        }
        glib::idle_add_once(move || {
            if GEN.load(Ordering::SeqCst) == gen {
                if let Some(b) = handle() {
                    b.set_images(paths);
                }
            }
        });
    });
}

/// Page switched: pick the right mode and the best remembered context.
pub fn page_changed(name: &str) {
    let hero = matches!(name, "home" | "stage" | "performer");
    let mode = if hero { Mode::Sharp } else { Mode::Ambient };
    if let Some(b) = handle() {
        b.set_mode(mode);
    }
    // Library/performer pages set their own context when they open an item.
    if name == "performer" {
        return;
    }
    let ctx = PLAYING.with(|p| p.borrow().clone()).or_else(|| LAST.with(|l| l.borrow().clone()));
    if let Some(ctx) = ctx {
        let same = LAST.with(|l| l.borrow().as_ref() == Some(&ctx));
        if !same || hero {
            set_context(mode, ctx);
        }
    }
}

/// Worker-thread: cover (HD) + artist fan art, all quality-gated.
fn resolve_paths(ctx: &Ctx, mode: Mode, mbid: Option<&str>, wiki: Option<&str>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    // Album cover first when we have one (album pages / now playing).
    let cover = if ctx.album.is_empty() {
        None
    } else {
        crate::artwork::hero_cover(&ctx.artist, &ctx.album, ctx.cover_url.clone())
    };
    let artist_only = ctx.album.is_empty();
    // For an artist context the server cover is a small thumbnail: use it last.
    let cover = if artist_only { None } else { cover.or_else(|| ctx.cover_url.clone()) };
    if let Some(url) = cover.map(|u| artimg::cap_url(&u)) {
        if let Ok(p) = crate::artwork::fetch_image_file(&url) {
            out.push(p);
        }
    }
    if !ctx.artist.is_empty() {
        let imgs = artimg::artist_images(&ctx.artist, mbid, wiki);
        let want = if mode == Mode::Sharp { 5 } else { 2 };
        let mut got = 0;
        for c in imgs.backdrops() {
            if got >= want {
                break;
            }
            let use_ = if c.kind == Kind::Portrait { Use::Portrait } else { Use::Backdrop };
            if let Some(p) = artimg::fetch_good(c, use_) {
                if !out.contains(&p) {
                    out.push(p);
                    got += 1;
                }
            }
        }
        if out.is_empty() && artist_only {
            if let Some(url) = ctx.cover_url.clone() {
                if let Ok(p) = crate::artwork::fetch_image_file(&artimg::cap_url(&url)) {
                    out.push(p);
                }
            }
        }
        if out.is_empty() {
            if let Some(c) = imgs.portrait() {
                if let Some(p) = artimg::fetch_good(c, Use::Portrait) {
                    out.push(p);
                }
            }
        }
    }
    out
}
