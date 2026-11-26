//! Cross-source likes → local taste profile (no Google sync).

use crate::model::{SourceKind, Track};
use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct TasteProfile {
    conn: Mutex<Connection>,
}

fn db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neighbourhood-music")
        .join("taste.sqlite3")
}

impl TasteProfile {
    pub fn open() -> Result<Self> {
        let path = db_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS likes (
                track_id TEXT PRIMARY KEY,
                title TEXT,
                artist TEXT,
                album TEXT,
                source TEXT,
                liked_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS artists_affinity (
                artist TEXT PRIMARY KEY,
                score REAL NOT NULL
            );",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn like(&self, track: &Track) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let source = format!("{:?}", track.source);
        self.conn.lock().unwrap().execute(
            "INSERT INTO likes(track_id,title,artist,album,source,liked_at)
             VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(track_id) DO UPDATE SET liked_at=excluded.liked_at",
            params![track.id, track.title, track.artist, track.album, source, now],
        )?;
        self.bump_artist(&track.artist, 1.0)?;
        Ok(())
    }

    pub fn unlike(&self, track_id: &str) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM likes WHERE track_id=?1", params![track_id])?;
        Ok(())
    }

    pub fn is_liked(&self, track_id: &str) -> bool {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT 1 FROM likes WHERE track_id=?1",
                params![track_id],
                |_| Ok(()),
            )
            .is_ok()
    }

    pub fn liked_tracks(&self, limit: usize) -> Result<Vec<(String, String, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT track_id, title, artist, album FROM likes ORDER BY liked_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    fn bump_artist(&self, artist: &str, delta: f64) -> Result<()> {
        if artist.is_empty() {
            return Ok(());
        }
        self.conn.lock().unwrap().execute(
            "INSERT INTO artists_affinity(artist, score) VALUES(?1,?2)
             ON CONFLICT(artist) DO UPDATE SET score=score+excluded.score",
            params![artist, delta],
        )?;
        Ok(())
    }

    pub fn top_artists(&self, limit: usize) -> Result<Vec<(String, f64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT artist, score FROM artists_affinity ORDER BY score DESC LIMIT ?1")?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

pub fn source_label(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Server => "Server",
        SourceKind::YoutubeMusic => "YouTube Music",
        SourceKind::Local => "Local",
        SourceKind::Merged => "Merged",
    }
}
