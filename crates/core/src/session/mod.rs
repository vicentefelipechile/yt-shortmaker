// =================================================================================================
// session — SQLite session persistence (video-centric, no projects list)
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
-- Per-video processing state (replaces projects). One row per video_id.
CREATE TABLE IF NOT EXISTS video_jobs (
  video_id TEXT PRIMARY KEY,
  url TEXT NOT NULL,
  title TEXT NOT NULL,
  duration REAL NOT NULL,
  work_dir TEXT NOT NULL,
  download_path TEXT,
  download_verified INTEGER NOT NULL DEFAULT 0,
  split_verified INTEGER NOT NULL DEFAULT 0,
  total_chunks INTEGER NOT NULL DEFAULT 0,
  analyzed_chunks INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS job_chunks (
  video_id TEXT NOT NULL,
  idx INTEGER NOT NULL,
  start_sec INTEGER NOT NULL,
  path TEXT NOT NULL,
  status TEXT NOT NULL,
  PRIMARY KEY (video_id, idx)
);
CREATE TABLE IF NOT EXISTS job_moments (
  video_id TEXT NOT NULL,
  idx INTEGER NOT NULL,
  start_time TEXT NOT NULL,
  end_time TEXT NOT NULL,
  category TEXT NOT NULL,
  description TEXT NOT NULL,
  dialogue_json TEXT NOT NULL,
  PRIMARY KEY (video_id, idx)
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
CREATE INDEX IF NOT EXISTS idx_job_chunks_video ON job_chunks(video_id);
CREATE INDEX IF NOT EXISTS idx_job_moments_video ON job_moments(video_id);
"#;

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VideoJob {
    pub video_id: String,
    pub url: String,
    pub title: String,
    pub duration: f64,
    pub work_dir: String,
    pub download_path: Option<String>,
    pub download_verified: bool,
    pub split_verified: bool,
    pub total_chunks: i64,
    pub analyzed_chunks: i64,
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
    // Best-effort cleanup of legacy tables (no retrocompat per user request)
    let _ = conn.execute_batch(
        "DROP TABLE IF EXISTS projects; DROP TABLE IF EXISTS chunks; DROP TABLE IF EXISTS moments; DROP TABLE IF EXISTS exports;",
    );
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
    let safe = sanitize_id(video_id);
    Ok(img_cache_dir()?.join(format!("{safe}.jpg")))
}

/// Persistent work dir for a given video_id (verified on resume).
pub fn video_work_dir(video_id: &str) -> Result<PathBuf> {
    let safe = sanitize_id(video_id);
    Ok(db_dir()?.join("videos").join(safe))
}

pub fn video_download_path(video_id: &str) -> Result<PathBuf> {
    Ok(video_work_dir(video_id)?.join(format!("{}.mp4", sanitize_id(video_id))))
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// -------------------------------------------------------------------------------------------------
// Video jobs (checkpoint state)
// -------------------------------------------------------------------------------------------------

pub fn upsert_video_job(conn: &Connection, job: &VideoJob) -> Result<()> {
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO video_jobs (video_id, url, title, duration, work_dir, download_path, download_verified, split_verified, total_chunks, analyzed_chunks, status, created_at, updated_at)
          VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
          ON CONFLICT(video_id) DO UPDATE SET
            url=excluded.url, title=excluded.title, duration=excluded.duration, work_dir=excluded.work_dir,
            download_path=excluded.download_path, download_verified=excluded.download_verified,
            split_verified=excluded.split_verified, total_chunks=excluded.total_chunks,
            analyzed_chunks=excluded.analyzed_chunks, status=excluded.status, updated_at=excluded.updated_at",
        params![
            job.video_id,
            job.url,
            job.title,
            job.duration,
            job.work_dir,
            job.download_path,
            job.download_verified as i64,
            job.split_verified as i64,
            job.total_chunks,
            job.analyzed_chunks,
            job.status,
            now
        ],
    )
    .context("upserting video job")?;
    Ok(())
}

pub fn get_video_job(conn: &Connection, video_id: &str) -> Result<Option<VideoJob>> {
    conn.query_row(
        "SELECT video_id, url, title, duration, work_dir, download_path, download_verified, split_verified, total_chunks, analyzed_chunks, status FROM video_jobs WHERE video_id = ?1",
        params![video_id],
        |row| {
            Ok(VideoJob {
                video_id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                duration: row.get(3)?,
                work_dir: row.get(4)?,
                download_path: row.get(5)?,
                download_verified: row.get::<_, i64>(6)? != 0,
                split_verified: row.get::<_, i64>(7)? != 0,
                total_chunks: row.get(8)?,
                analyzed_chunks: row.get(9)?,
                status: row.get(10)?,
            })
        },
    )
    .optional()
    .context("querying video job")
}

pub fn get_job_chunks(conn: &Connection, video_id: &str) -> Result<Vec<VideoChunk>> {
    let mut stmt = conn
        .prepare("SELECT path, start_sec FROM job_chunks WHERE video_id = ?1 ORDER BY idx")
        .context("preparing get job chunks")?;
    let rows = stmt
        .query_map(params![video_id], |row| {
            let path: String = row.get(0)?;
            let start: i64 = row.get(1)?;
            Ok(VideoChunk {
                file_path: path,
                start_seconds: start as u64,
            })
        })
        .context("querying job chunks")?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.context("mapping chunk row")?);
    }
    Ok(out)
}

pub fn get_job_chunk_status(conn: &Connection, video_id: &str) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn
        .prepare("SELECT idx, status FROM job_chunks WHERE video_id = ?1 ORDER BY idx")
        .context("preparing chunk status")?;
    let rows = stmt
        .query_map(params![video_id], |row| {
            let idx: i64 = row.get(0)?;
            let status: String = row.get(1)?;
            Ok((idx, status))
        })
        .context("querying chunk status")?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.context("mapping status")?);
    }
    Ok(out)
}

pub fn insert_job_chunks(conn: &Connection, video_id: &str, chunks: &[VideoChunk]) -> Result<()> {
    let tx = conn.unchecked_transaction().context("starting chunks tx")?;
    tx.execute(
        "DELETE FROM job_chunks WHERE video_id = ?1",
        params![video_id],
    )
    .context("clearing old chunks")?;
    for (idx, c) in chunks.iter().enumerate() {
        tx.execute(
            "INSERT INTO job_chunks (video_id, idx, start_sec, path, status) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![video_id, idx as i64, c.start_seconds as i64, c.file_path, "split_ok"],
        )
        .context("inserting job chunk")?;
    }
    tx.commit().context("committing chunks")?;
    Ok(())
}

pub fn update_job_chunk_status(
    conn: &Connection,
    video_id: &str,
    idx: i64,
    status: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE job_chunks SET status = ?1 WHERE video_id = ?2 AND idx = ?3",
        params![status, video_id, idx],
    )
    .context("updating chunk status")?;
    Ok(())
}

pub fn get_job_moments(conn: &Connection, video_id: &str) -> Result<Vec<VideoMoment>> {
    let mut stmt = conn
        .prepare(
            "SELECT start_time, end_time, category, description, dialogue_json FROM job_moments WHERE video_id = ?1 ORDER BY idx",
        )
        .context("preparing get moments")?;
    let rows = stmt
        .query_map(params![video_id], |row| {
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
    for r in rows {
        out.push(r.context("mapping moment")?);
    }
    Ok(out)
}

pub fn insert_job_moments(
    conn: &Connection,
    video_id: &str,
    moments: &[VideoMoment],
) -> Result<()> {
    let tx = conn
        .unchecked_transaction()
        .context("starting moments tx")?;
    tx.execute(
        "DELETE FROM job_moments WHERE video_id = ?1",
        params![video_id],
    )
    .context("clearing moments")?;
    for (idx, m) in moments.iter().enumerate() {
        let dialogue_json = serde_json::to_string(&m.dialogue).context("serializing dialogue")?;
        tx.execute(
            "INSERT INTO job_moments (video_id, idx, start_time, end_time, category, description, dialogue_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![video_id, idx as i64, m.start_time, m.end_time, m.category, m.description, dialogue_json],
        )
        .context("inserting moment")?;
    }
    tx.commit().context("committing moments")?;
    Ok(())
}

pub fn append_job_moments(
    conn: &Connection,
    video_id: &str,
    new_moments: &[VideoMoment],
) -> Result<()> {
    if new_moments.is_empty() {
        return Ok(());
    }
    let existing = get_job_moments(conn, video_id)?.len() as i64;
    let tx = conn.unchecked_transaction().context("starting append tx")?;
    for (i, m) in new_moments.iter().enumerate() {
        let idx = existing + i as i64;
        let dialogue_json = serde_json::to_string(&m.dialogue).context("serializing dialogue")?;
        tx.execute(
            "INSERT OR REPLACE INTO job_moments (video_id, idx, start_time, end_time, category, description, dialogue_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![video_id, idx, m.start_time, m.end_time, m.category, m.description, dialogue_json],
        )
        .context("appending moment")?;
    }
    tx.commit().context("committing append")?;
    Ok(())
}

pub fn delete_video_job(conn: &Connection, video_id: &str) -> Result<()> {
    let tx = conn.unchecked_transaction().context("starting delete tx")?;
    tx.execute(
        "DELETE FROM job_chunks WHERE video_id = ?1",
        params![video_id],
    )
    .context("deleting chunks")?;
    tx.execute(
        "DELETE FROM job_moments WHERE video_id = ?1",
        params![video_id],
    )
    .context("deleting moments")?;
    tx.execute(
        "DELETE FROM video_jobs WHERE video_id = ?1",
        params![video_id],
    )
    .context("deleting job")?;
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
    let path = dir.join(format!("{}.jpg", sanitize_id(video_id)));
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
    fn test_video_job_crud() {
        let dir = tempdir().unwrap();
        let conn = init_db(&dir.path().join("test.db")).unwrap();
        let job = VideoJob {
            video_id: "abc123".into(),
            url: "https://youtube.com/watch?v=abc123".into(),
            title: "Test".into(),
            duration: 100.0,
            work_dir: "/tmp/work".into(),
            download_path: Some("/tmp/work/abc123.mp4".into()),
            download_verified: true,
            split_verified: false,
            total_chunks: 3,
            analyzed_chunks: 1,
            status: "splitting".into(),
        };
        upsert_video_job(&conn, &job).unwrap();
        let loaded = get_video_job(&conn, "abc123").unwrap().unwrap();
        assert_eq!(loaded.title, "Test");
        assert!(loaded.download_verified);
        assert_eq!(loaded.total_chunks, 3);
        delete_video_job(&conn, "abc123").unwrap();
        assert!(get_video_job(&conn, "abc123").unwrap().is_none());
    }

    #[test]
    fn test_chunks_and_moments_roundtrip() {
        let dir = tempdir().unwrap();
        let conn = init_db(&dir.path().join("test.db")).unwrap();
        let vid = "vid123";
        let job = VideoJob {
            video_id: vid.into(),
            url: "https://youtube.com/watch?v=vid123".into(),
            title: "T".into(),
            duration: 1200.0,
            work_dir: "/tmp/w".into(),
            download_path: None,
            download_verified: false,
            split_verified: false,
            total_chunks: 2,
            analyzed_chunks: 0,
            status: "pending".into(),
        };
        upsert_video_job(&conn, &job).unwrap();
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
        insert_job_chunks(&conn, vid, &chunks).unwrap();
        let loaded_chunks = get_job_chunks(&conn, vid).unwrap();
        assert_eq!(loaded_chunks.len(), 2);
        let moments = sample_moments();
        insert_job_moments(&conn, vid, &moments).unwrap();
        let loaded = get_job_moments(&conn, vid).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[1].dialogue.len(), 1);
    }

    #[test]
    fn test_append_moments() {
        let dir = tempdir().unwrap();
        let conn = init_db(&dir.path().join("test.db")).unwrap();
        let vid = "append1";
        let m1 = sample_moments()[0].clone();
        let m2 = sample_moments()[1].clone();
        insert_job_moments(&conn, vid, std::slice::from_ref(&m1)).unwrap();
        append_job_moments(&conn, vid, std::slice::from_ref(&m2)).unwrap();
        let loaded = get_job_moments(&conn, vid).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].start_time, "00:00:05");
        assert_eq!(loaded[1].start_time, "00:01:00");
    }

    #[test]
    fn test_video_cache_and_img_cache() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("nested").join("session.db");
        let conn = init_db(&db).unwrap();
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

        let mut cv2 = cv.clone();
        cv2.title = "Updated".into();
        upsert_cached_video(&conn, &cv2).unwrap();
        let loaded2 = get_cached_video(&conn, &cv.url).unwrap().unwrap();
        assert_eq!(loaded2.title, "Updated");

        let by_id = get_cached_video_by_id(&conn, "abc123DEF45")
            .unwrap()
            .unwrap();
        assert_eq!(by_id.url, cv.url);
    }

    #[test]
    fn test_thumbnail_cache_path_is_under_db_dir() {
        let p = thumbnail_cache_path("dQw4w9WgXcQ").unwrap();
        assert_eq!(p.file_name().unwrap().to_string_lossy(), "dQw4w9WgXcQ.jpg");
        assert_eq!(
            p.parent().unwrap().file_name().unwrap().to_string_lossy(),
            "img_cache"
        );
    }

    #[test]
    fn test_video_work_dir_is_under_db_dir() {
        let p = video_work_dir("dQw4w9WgXcQ").unwrap();
        assert!(p.to_string_lossy().contains("videos"));
        assert_eq!(p.file_name().unwrap().to_string_lossy(), "dQw4w9WgXcQ");
    }
}
