//! Startup snapshots of the database (fork addition).
//!
//! Every launch copies the database into `{app_data_dir}/backups/` *before*
//! migrations run, so a new build can never damage study records without
//! leaving a restore point. `VACUUM INTO` writes a consistent, self-contained
//! file even while WAL mode is active.
//!
//! Retention: the newest `KEEP_RECENT` snapshots, plus the newest snapshot of
//! each of the last `KEEP_DAYS` days that have one.
//!
//! To restore: quit the app, then copy a snapshot over `pomopipen.db` and
//! delete `pomopipen.db-wal` / `pomopipen.db-shm`.

use rusqlite::Connection;
use std::collections::HashSet;
use std::path::Path;

const KEEP_RECENT: usize = 10;
const KEEP_DAYS: usize = 30;
const PREFIX: &str = "pomopipen-";

/// Take a snapshot and prune old ones. Never blocks startup: errors are logged.
pub fn snapshot(conn: &Connection, app_data_dir: &Path) {
    if let Err(e) = try_snapshot(conn, app_data_dir) {
        log::warn!("[db/backup] snapshot failed: {e}");
    }
}

fn try_snapshot(conn: &Connection, app_data_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // A brand-new database has nothing worth keeping yet.
    let has_sessions: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='sessions'",
        [],
        |row| row.get(0),
    )?;
    if !has_sessions {
        return Ok(());
    }

    let dir = app_data_dir.join("backups");
    std::fs::create_dir_all(&dir)?;
    let stamp: String = conn.query_row(
        "SELECT strftime('%Y%m%d-%H%M%S', 'now', 'localtime')",
        [],
        |row| row.get(0),
    )?;
    let path = dir.join(format!("{PREFIX}{stamp}.db"));
    if !path.exists() {
        let target = path.to_string_lossy().into_owned();
        conn.execute("VACUUM INTO ?1", [&target])?;
        log::info!("[db/backup] snapshot written: {target}");
    }

    let mut names: Vec<String> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with(PREFIX) && n.ends_with(".db"))
        .collect();
    names.sort_unstable_by(|a, b| b.cmp(a));
    for name in expired(&names) {
        if let Err(e) = std::fs::remove_file(dir.join(name)) {
            log::warn!("[db/backup] could not prune {name}: {e}");
        }
    }
    Ok(())
}

/// `newest_first` holds `pomopipen-YYYYMMDD-HHMMSS.db` names sorted newest
/// first; returns the ones the retention policy drops.
fn expired(newest_first: &[String]) -> Vec<&String> {
    let mut days_kept: HashSet<&str> = HashSet::new();
    let mut doomed = Vec::new();
    for (i, name) in newest_first.iter().enumerate() {
        let day = name.get(PREFIX.len()..PREFIX.len() + 8).unwrap_or_default();
        let newest_of_day = days_kept.len() < KEEP_DAYS && days_kept.insert(day);
        if i >= KEEP_RECENT && !newest_of_day {
            doomed.push(name);
        }
    }
    doomed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(day: u32, time: u32) -> String {
        format!("{PREFIX}{day:08}-{time:06}.db")
    }

    #[test]
    fn keeps_recent_and_one_per_day() {
        // 15 launches on one day, newest first.
        let mut names: Vec<String> = (0..15).rev().map(|t| name(20260923, t)).collect();
        names.push(name(20260922, 120000));
        names.push(name(20260922, 90000));
        let doomed = expired(&names);
        // Keep the newest 10 of 2026-09-23 and the newest of 2026-09-22.
        assert_eq!(doomed.len(), 6);
        assert!(!doomed.contains(&&name(20260922, 120000)));
        assert!(doomed.contains(&&name(20260922, 90000)));
        assert!(doomed.contains(&&name(20260923, 0)));
    }

    #[test]
    fn keeps_at_most_thirty_days() {
        let names: Vec<String> = (0..40).rev().map(|d| name(20260100 + d, 0)).collect();
        assert_eq!(expired(&names).len(), 10);
    }

    #[test]
    fn snapshot_skips_empty_db_and_copies_real_one() {
        let dir = std::env::temp_dir().join(format!("pomopipen-backup-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let conn = Connection::open(dir.join("pomopipen.db")).unwrap();

        snapshot(&conn, &dir);
        assert!(!dir.join("backups").exists(), "empty database must not be backed up");

        crate::db::migrations::run(&conn).unwrap();
        snapshot(&conn, &dir);
        let copies: Vec<_> = std::fs::read_dir(dir.join("backups")).unwrap().collect();
        assert_eq!(copies.len(), 1);
        let copy = Connection::open(copies[0].as_ref().unwrap().path()).unwrap();
        let tables: i64 = copy
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name='sessions'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tables, 1);

        drop(copy);
        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
