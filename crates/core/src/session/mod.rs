// =================================================================================================
// session — SQLite session persistence
// =================================================================================================

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

use crate::types::VideoChunk;
use crate::types::VideoMoment;

// -------------------------------------------------------------------------------------------------
// Constants
// -------------------------------------------------------------------------------------------------

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS projects (
  id TEXT PRIMARY KEY,
  url TEXT NOT NULL,
  video_id TEXT NOT NULL,
  title TEXT NOT NULL,
  duration REAL NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS chunks (
  project_id TEXT NOT NULL,
  idx INTEGER NOT NULL,
  start_sec INTEGER NOT NULL,
  path TEXT NOT NULL,
  PRIMARY KEY (project_id, idx)
);
CREATE TABLE IF NOT EXISTS moments (
  project_id TEXT NOT NULL,
  idx INTEGER NOT NULL,
  start_time TEXT NOT NULL,
  end_time TEXT NOT NULL,
  category TEXT NOT NULL,
  description TEXT NOT NULL,
  dialogue_json TEXT NOT NULL,
  PRIMARY KEY (project_id, idx)
);
CREATE TABLE IF NOT EXISTS exports (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  plano_json TEXT NOT NULL,
  output_dir TEXT NOT NULL,
  created_at TEXT NOT NULL
);
-- Cache for fetch_info (ytdlp) so repeated URLs do not hit the network
CREATE TABLE IF NOT EXISTS video_cache (
  url TEXT PRIMARY KEY,
  video_id TEXT NOT NULL,
  title TEXT NOT NULL,
  duration REAL NOT NULL,
  thumbnail_url TEXT,
  thumbnail_path TEXT,
  fetched_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_video_cache_video_id ON video_cache(video_id);
"#;

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Project {
    pub id: String,
    pub url: String,
    pub video_id: String,
    pub title: String,
    pub duration: f64,
    pub status: String,
}

/// Cached VideoInfo persisted in SQLite (same folder as session.db).
/// Thumbnail bytes are stored on disk under `img_cache/` next to the DB.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedVideo {
    pub url: String,
    pub video_id: String,
    pub title: String,
    pub duration: f64,
    pub thumbnail_url: Option<String>,
    /// Absolute path on disk under `img_cache/` (if thumbnail was cached).
    pub thumbnail_path: Option<String>,
    pub fetched_at: String,
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

pub fn init_db(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("creating db dir")?;
    }
    let conn = Connection::open(path).context("opening sqlite")?;
    conn.execute_batch(SCHEMA).context("creating schema")?;
    // Ensure img_cache exists alongside the DB (same parent dir)
    if let Some(parent) = path.parent() {
        let cache = parent.join("img_cache");
        let _ = std::fs::create_dir_all(&cache);
    }
    Ok(conn)
}

pub fn db_path() -> Result<std::path::PathBuf> {
    let dir = dirs::data_local_dir().context("resolving data_local_dir")?;
    Ok(dir.join("yt-shortmaker-v2").join("session.db"))
}

/// Directory that holds `session.db` (e.g. `%LOCALAPPDATA%/yt-shortmaker-v2` on Windows).
pub fn db_dir() -> Result<PathBuf> {
    Ok(db_path()?
        .parent()
        .context("db_path has no parent")?
        .to_path_buf())
}

/// `img_cache/` lives in the **same directory** as `session.db` — all thumbnails and image
/// content are persisted here so the whole session is cacheable.
pub fn img_cache_dir() -> Result<PathBuf> {
    Ok(db_dir()?.join("img_cache"))
}

pub fn ensure_img_cache_dir() -> Result<PathBuf> {
    let dir = img_cache_dir()?;
    std::fs::create_dir_all(&dir).context("creating img_cache dir")?;
    Ok(dir)
}

/// Deterministic thumbnail path for a given video_id (always under `img_cache/`).
/// Extension is normalised to `.jpg` — yt-dlp thumbnails are almost always jpeg.
pub fn thumbnail_cache_path(video_id: &str) -> Result<PathBuf> {
    let safe: String = video_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    Ok(img_cache_dir()?.join(format!("{safe}.jpg")))
}

pub fn insert_project(conn: &Connection, project: &Project) -> Result<()> {
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO projects (id, url, video_id, title, duration, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
         ON CONFLICT(id) DO UPDATE SET
           url=excluded.url, video_id=excluded.video_id, title=excluded.title,
           duration=excluded.duration, status=excluded.status, updated_at=excluded.updated_at",
        params![
            project.id,
            project.url,
            project.video_id,
            project.title,
            project.duration,
            project.status,
            now
        ],
    )
    .context("inserting project")?;
    Ok(())
}

pub fn insert_chunks(conn: &Connection, project_id: &str, chunks: &[VideoChunk]) -> Result<()> {
    let tx = conn.unchecked_transaction().context("starting chunks tx")?;
    for (idx, c) in chunks.iter().enumerate() {
        tx.execute(
            "INSERT OR REPLACE INTO chunks (project_id, idx, start_sec, path) VALUES (?1, ?2, ?3, ?4)",
            params![project_id, idx as i64, c.start_seconds as i64, c.file_path],
        )
        .context("inserting chunk")?;
    }
    tx.commit().context("committing chunks")?;
    Ok(())
}

pub fn insert_moments(conn: &Connection, project_id: &str, moments: &[VideoMoment]) -> Result<()> {
    let tx = conn
        .unchecked_transaction()
        .context("starting moments tx")?;
    for (idx, m) in moments.iter().enumerate() {
        let dialogue_json = serde_json::to_string(&m.dialogue).context("serializing dialogue")?;
        tx.execute(
            "INSERT OR REPLACE INTO moments
             (project_id, idx, start_time, end_time, category, description, dialogue_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                project_id,
                idx as i64,
                m.start_time,
                m.end_time,
                m.category,
                m.description,
                dialogue_json
            ],
        )
        .context("inserting moment")?;
    }
    tx.commit().context("committing moments")?;
    Ok(())
}

pub fn list_projects(conn: &Connection) -> Result<Vec<Project>> {
    let mut stmt = conn
        .prepare("SELECT id, url, video_id, title, duration, status FROM projects ORDER BY created_at DESC")
        .context("preparing list projects")?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                url: row.get(1)?,
                video_id: row.get(2)?,
                title: row.get(3)?,
                duration: row.get(4)?,
                status: row.get(5)?,
            })
        })
        .context("querying projects")?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.context("mapping project row")?);
    }
    Ok(out)
}

pub fn get_project(conn: &Connection, id: &str) -> Result<Option<Project>> {
    conn.query_row(
        "SELECT id, url, video_id, title, duration, status FROM projects WHERE id = ?1",
        params![id],
        |row| {
            Ok(Project {
                id: row.get(0)?,
                url: row.get(1)?,
                video_id: row.get(2)?,
                title: row.get(3)?,
                duration: row.get(4)?,
                status: row.get(5)?,
            })
        },
    )
    .optional()
    .context("querying project")
}

pub fn get_moments(conn: &Connection, project_id: &str) -> Result<Vec<VideoMoment>> {
    let mut stmt = conn
        .prepare(
            "SELECT start_time, end_time, category, description, dialogue_json
             FROM moments WHERE project_id = ?1 ORDER BY idx",
        )
        .context("preparing get moments")?;
    let rows = stmt
        .query_map(params![project_id], |row| {
            let dialogue_json: String = row.get(4)?;
            let dialogue = serde_json::from_str(&dialogue_json).unwrap_or_default();
            Ok(VideoMoment {
                start_time: row.get(0)?,
                end_time: row.get(1)?,
                category: row.get(2)?,
                description: row.get(3)?,
                dialogue,
            })
        })
        .context("querying moments")?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.context("mapping moment row")?);
    }
    Ok(out)
}

pub fn delete_project(conn: &Connection, id: &str) -> Result<()> {
    let tx = conn.unchecked_transaction().context("starting delete tx")?;
    tx.execute("DELETE FROM chunks WHERE project_id = ?1", params![id])
        .context("deleting chunks")?;
    tx.execute("DELETE FROM moments WHERE project_id = ?1", params![id])
        .context("deleting moments")?;
    tx.execute("DELETE FROM projects WHERE id = ?1", params![id])
        .context("deleting project")?;
    tx.commit().context("committing delete")?;
    Ok(())
}

// -------------------------------------------------------------------------------------------------
// Video cache (fetch_info) — SQLite + img_cache/
// -------------------------------------------------------------------------------------------------

pub fn upsert_cached_video(conn: &Connection, cached: &CachedVideo) -> Result<()> {
    conn.execute(
        "INSERT INTO video_cache (url, video_id, title, duration, thumbnail_url, thumbnail_path, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(url) DO UPDATE SET
           video_id=excluded.video_id, title=excluded.title, duration=excluded.duration,
           thumbnail_url=excluded.thumbnail_url, thumbnail_path=excluded.thumbnail_path,
           fetched_at=excluded.fetched_at",
        params![
            cached.url,
            cached.video_id,
            cached.title,
            cached.duration,
            cached.thumbnail_url,
            cached.thumbnail_path,
            cached.fetched_at
        ],
    )
    .context("upserting cached video")?;
    Ok(())
}

pub fn get_cached_video(conn: &Connection, url: &str) -> Result<Option<CachedVideo>> {
    conn.query_row(
        "SELECT url, video_id, title, duration, thumbnail_url, thumbnail_path, fetched_at
         FROM video_cache WHERE url = ?1",
        params![url],
        |row| {
            Ok(CachedVideo {
                url: row.get(0)?,
                video_id: row.get(1)?,
                title: row.get(2)?,
                duration: row.get(3)?,
                thumbnail_url: row.get(4)?,
                thumbnail_path: row.get(5)?,
                fetched_at: row.get(6)?,
            })
        },
    )
    .optional()
    .context("querying cached video")
}

pub fn get_cached_video_by_id(conn: &Connection, video_id: &str) -> Result<Option<CachedVideo>> {
    conn.query_row(
        "SELECT url, video_id, title, duration, thumbnail_url, thumbnail_path, fetched_at
         FROM video_cache WHERE video_id = ?1 ORDER BY fetched_at DESC LIMIT 1",
        params![video_id],
        |row| {
            Ok(CachedVideo {
                url: row.get(0)?,
                video_id: row.get(1)?,
                title: row.get(2)?,
                duration: row.get(3)?,
                thumbnail_url: row.get(4)?,
                thumbnail_path: row.get(5)?,
                fetched_at: row.get(6)?,
            })
        },
    )
    .optional()
    .context("querying cached video by id")
}

/// Stores thumbnail bytes to `img_cache/<video_id>.jpg` and returns the absolute path.
/// Caller decides when to update the DB row; this only touches the filesystem.
pub fn save_thumbnail_to_cache(video_id: &str, bytes: &[u8]) -> Result<PathBuf> {
    let dir = ensure_img_cache_dir()?;
    let path = dir.join(format!(
        "{}.jpg",
        video_id
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            })
            .collect::<String>()
    ));
    std::fs::write(&path, bytes).context("writing thumbnail to img_cache")?;
    Ok(path)
}

/// Returns cached thumbnail path if the file actually exists on disk.
pub fn cached_thumbnail_on_disk(video_id: &str) -> Option<PathBuf> {
    let path = thumbnail_cache_path(video_id).ok()?;
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_moments() -> Vec<VideoMoment> {
        vec![
            VideoMoment {
                start_time: "00:00:05".into(),
                end_time: "00:00:15".into(),
                category: "hook".into(),
                description: "opening hook".into(),
                dialogue: vec![],
            },
            VideoMoment {
                start_time: "00:01:00".into(),
                end_time: "00:01:20".into(),
                category: "funny".into(),
                description: "joke".into(),
                dialogue: vec![crate::types::DialoguePhrase {
                    start_time: "00:01:00".into(),
                    end_time: "00:01:02".into(),
                    phrase: "hello world".into(),
                }],
            },
        ]
    }

    #[test]
    fn test_project_crud() {
        let dir = tempdir().unwrap();
        let conn = init_db(&dir.path().join("test.db")).unwrap();
        let project = Project {
            id: "p1".into(),
            url: "https://youtube.com/watch?v=abc".into(),
            video_id: "abc".into(),
            title: "Test".into(),
            duration: 100.0,
            status: "analyzed".into(),
        };
        insert_project(&conn, &project).unwrap();
        let loaded = get_project(&conn, "p1").unwrap().unwrap();
        assert_eq!(loaded.title, "Test");
        assert_eq!(list_projects(&conn).unwrap().len(), 1);
        delete_project(&conn, "p1").unwrap();
        assert!(get_project(&conn, "p1").unwrap().is_none());
    }

    #[test]
    fn test_chunks_and_moments_roundtrip() {
        let dir = tempdir().unwrap();
        let conn = init_db(&dir.path().join("test.db")).unwrap();
        let chunks = vec![
            VideoChunk {
                start_seconds: 0,
                file_path: "/tmp/c0.mp4".into(),
            },
            VideoChunk {
                start_seconds: 600,
                file_path: "/tmp/c1.mp4".into(),
            },
        ];
        insert_chunks(&conn, "p1", &chunks).unwrap();
        let moments = sample_moments();
        insert_moments(&conn, "p1", &moments).unwrap();

        let loaded = get_moments(&conn, "p1").unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[1].dialogue.len(), 1);
        assert_eq!(loaded[1].dialogue[0].phrase, "hello world");
    }

    #[test]
    fn test_reinsert_replaces_by_idx() {
        let dir = tempdir().unwrap();
        let conn = init_db(&dir.path().join("test.db")).unwrap();
        let moments = sample_moments();
        insert_moments(&conn, "p1", &moments).unwrap();

        let mut updated = moments[0].clone();
        updated.start_time = "00:00:09".into();
        insert_moments(&conn, "p1", &[updated]).unwrap();

        let loaded = get_moments(&conn, "p1").unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].start_time, "00:00:09");
        assert_eq!(loaded[1].start_time, "00:01:00");
    }

    #[test]
    fn test_video_cache_and_img_cache() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("nested").join("session.db");
        let conn = init_db(&db).unwrap();
        // init_db must have created img_cache alongside the DB
        assert!(db.parent().unwrap().join("img_cache").exists());

        let cv = CachedVideo {
            url: "https://youtube.com/watch?v=abc123DEF45".into(),
            video_id: "abc123DEF45".into(),
            title: "Cached Title".into(),
            duration: 123.0,
            thumbnail_url: Some("https://example.com/thumb.jpg".into()),
            thumbnail_path: None,
            fetched_at: chrono::Local::now().to_rfc3339(),
        };
        upsert_cached_video(&conn, &cv).unwrap();
        let loaded = get_cached_video(&conn, &cv.url).unwrap().unwrap();
        assert_eq!(loaded.title, "Cached Title");
        assert_eq!(loaded.video_id, "abc123DEF45");

        // Upsert should replace
        let mut cv2 = cv.clone();
        cv2.title = "Updated".into();
        upsert_cached_video(&conn, &cv2).unwrap();
        let loaded2 = get_cached_video(&conn, &cv.url).unwrap().unwrap();
        assert_eq!(loaded2.title, "Updated");

        // Lookup by video_id
        let by_id = get_cached_video_by_id(&conn, "abc123DEF45")
            .unwrap()
            .unwrap();
        assert_eq!(by_id.url, cv.url);
    }

    #[test]
    fn test_thumbnail_cache_path_is_under_db_dir() {
        // thumbnail_cache_path uses db_dir()/img_cache which resolves via dirs::data_local_dir,
        // so we only verify the components and that the function does not panic.
        let p = thumbnail_cache_path("dQw4w9WgXcQ").unwrap();
        assert_eq!(p.file_name().unwrap().to_string_lossy(), "dQw4w9WgXcQ.jpg");
        assert_eq!(
            p.parent().unwrap().file_name().unwrap().to_string_lossy(),
            "img_cache"
        );
    }
}
