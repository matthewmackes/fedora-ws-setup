use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct History {
    conn: Mutex<Connection>,
}

fn db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("neighbourhood-music")
        .join("history.sqlite3")
}

impl History {
    pub fn open() -> Result<Self> {
        let path = db_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS listens (
                id INTEGER PRIMARY KEY,
                server_id TEXT NOT NULL,
                track_id TEXT NOT NULL,
                title TEXT,
                artist TEXT,
                played_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn record_listen(
        &self,
        server_id: &str,
        track_id: &str,
        title: &str,
        artist: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.lock().unwrap().execute(
            "INSERT INTO listens (server_id, track_id, title, artist, played_at) VALUES (?1,?2,?3,?4,?5)",
            params![server_id, track_id, title, artist, now],
        )?;
        Ok(())
    }

    pub fn recent(&self, server_id: &str, limit: usize) -> Result<Vec<(String, String, String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT track_id, title, artist, played_at FROM listens WHERE server_id=?1 ORDER BY played_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![server_id, limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT value FROM settings WHERE key=?1",
                params![key],
                |row| row.get(0),
            )
            .ok()
    }
}
