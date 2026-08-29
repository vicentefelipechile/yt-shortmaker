// =================================================================================================
// session — SQLite session persistence
// =================================================================================================

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

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
