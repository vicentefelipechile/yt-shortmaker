// =================================================================================================
// session — SQLite session persistence
// =================================================================================================

use std::path::Path;

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

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

pub fn init_db(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("creating db dir")?;
    }
    let conn = Connection::open(path).context("opening sqlite")?;
    conn.execute_batch(SCHEMA).context("creating schema")?;
    Ok(conn)
}

pub fn db_path() -> Result<std::path::PathBuf> {
    let dir = dirs::data_local_dir().context("resolving data_local_dir")?;
    Ok(dir.join("yt-shortmaker-v2").join("session.db"))
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
}
