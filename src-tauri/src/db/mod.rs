pub mod backup;
pub mod migrations;
pub mod queries;
pub mod calendar;

use rusqlite::{Connection, Result};
use std::sync::{Arc, Mutex};

/// Thread-safe handle to the SQLite connection.
/// Registered as Tauri managed state so commands can access it.
pub type DbState = Arc<Mutex<Connection>>;

/// Open (or create) the `pomopipen.db` file inside `app_data_dir`,
/// enable WAL mode for better concurrent read performance,
/// run any pending schema migrations, and close rounds left open.
pub fn open(app_data_dir: &std::path::Path) -> Result<DbState> {
    let db_path = app_data_dir.join("pomopipen.db");
    let conn = Connection::open(&db_path)?;

    // WAL mode: readers don't block writers and vice-versa.
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

    // Restore point before this build's migrations touch anything.
    backup::snapshot(&conn, app_data_dir);
    migrations::run(&conn)?;
    // A round the last run never finished still counts the focus it saved.
    queries::close_abandoned_sessions(&conn)?;

    Ok(Arc::new(Mutex::new(conn)))
}
