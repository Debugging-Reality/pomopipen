use rusqlite::{Connection, Result};

/// Full schema for version 1. Tables use IF NOT EXISTS so the batch is
/// idempotent, but the schema_version check in `run()` prevents re-execution.
const MIGRATION_1: &str = "
    CREATE TABLE IF NOT EXISTS schema_version (
        version INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS settings (
        key   TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS sessions (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        started_at    INTEGER NOT NULL,
        ended_at      INTEGER,
        round_type    TEXT NOT NULL CHECK(round_type IN ('work', 'short-break', 'long-break')),
        duration_secs INTEGER NOT NULL CHECK(duration_secs > 0),
        completed     INTEGER NOT NULL DEFAULT 0 CHECK(completed IN (0, 1))
    );

    CREATE TABLE IF NOT EXISTS custom_themes (
        id     INTEGER PRIMARY KEY AUTOINCREMENT,
        name   TEXT NOT NULL UNIQUE,
        colors TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_sessions_started_at ON sessions(started_at);
    CREATE INDEX IF NOT EXISTS idx_sessions_round_type ON sessions(round_type);

    INSERT INTO schema_version VALUES (1);
";

/// Migrates timer duration storage from minute-resolution keys to second-resolution keys.
/// Reads existing `time_*_mins` rows, multiplies by 60, writes `time_*_secs`, then deletes
/// the old keys so key names align with the Settings struct field names.
const MIGRATION_2: &str = "
    INSERT OR IGNORE INTO settings (key, value)
        SELECT 'time_work_secs', CAST(CAST(value AS INTEGER) * 60 AS TEXT)
          FROM settings WHERE key = 'time_work_mins';
    INSERT OR IGNORE INTO settings (key, value)
        SELECT 'time_short_break_secs', CAST(CAST(value AS INTEGER) * 60 AS TEXT)
          FROM settings WHERE key = 'time_short_break_mins';
    INSERT OR IGNORE INTO settings (key, value)
        SELECT 'time_long_break_secs', CAST(CAST(value AS INTEGER) * 60 AS TEXT)
          FROM settings WHERE key = 'time_long_break_mins';
    DELETE FROM settings WHERE key IN
        ('time_work_mins', 'time_short_break_mins', 'time_long_break_mins');
    INSERT INTO schema_version VALUES (2);
";

/// Seeds the `check_for_updates` setting for users upgrading from a version
/// that did not have this setting. Fresh installs get it via seed_defaults.
const MIGRATION_3: &str = "
    INSERT OR IGNORE INTO settings (key, value) VALUES ('check_for_updates', 'true');
    INSERT INTO schema_version VALUES (3);
";

/// Seeds the `global_shortcuts_enabled` setting for all installs. Defaults to
/// 'false' — global shortcuts are now opt-in. This is a breaking change for
/// existing users who relied on shortcuts being active by default; they must
/// re-enable them in Settings → Shortcuts.
const MIGRATION_4: &str = "
    INSERT OR IGNORE INTO settings (key, value) VALUES ('global_shortcuts_enabled', 'false');
    INSERT INTO schema_version VALUES (4);
";

/// Seeds the `short_breaks_enabled` and `long_breaks_enabled` settings for
/// users upgrading from a version that did not have these settings.
/// Both default to 'true' — existing behaviour is preserved.
const MIGRATION_5: &str = "
    INSERT OR IGNORE INTO settings (key, value) VALUES ('short_breaks_enabled', 'true');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('long_breaks_enabled', 'true');
    INSERT INTO schema_version VALUES (5);
";

/// Seeds the seven local shortcut key bindings for users upgrading from a version
/// that did not have this feature. These shortcuts are handled entirely by the frontend
/// (keydown listeners) and require no Rust-side dispatch logic.
const MIGRATION_6: &str = "
    INSERT OR IGNORE INTO settings (key, value) VALUES ('local_shortcut_toggle', ' ');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('local_shortcut_reset', 'ArrowLeft');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('local_shortcut_skip', 'ArrowRight');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('local_shortcut_volume_down', 'ArrowDown');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('local_shortcut_volume_up', 'ArrowUp');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('local_shortcut_mute', 'm');
    INSERT OR IGNORE INTO settings (key, value) VALUES ('local_shortcut_fullscreen', 'F11');
    INSERT INTO schema_version VALUES (6);
";

/// Adds the `subjects` table and links work sessions to a subject.
///
/// `sessions.subject_id` is nullable on purpose: rows recorded before this
/// migration, and any round started without an active subject, stay NULL and
/// are reported as "uncategorised" in the stats views. There is deliberately
/// no id=0 "default subject" row — that would pollute the subject list.
///
/// ON DELETE SET NULL, never CASCADE: deleting a subject must not delete the
/// study history recorded against it.
const MIGRATION_7: &str = "
    CREATE TABLE IF NOT EXISTS subjects (
        id         INTEGER PRIMARY KEY AUTOINCREMENT,
        name       TEXT NOT NULL UNIQUE,
        color      TEXT NOT NULL,
        archived   INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0, 1)),
        created_at INTEGER NOT NULL,
        sort_order INTEGER NOT NULL DEFAULT 0
    );

    ALTER TABLE sessions ADD COLUMN subject_id INTEGER
        REFERENCES subjects(id) ON DELETE SET NULL;

    CREATE INDEX IF NOT EXISTS idx_sessions_subject ON sessions(subject_id);

    INSERT INTO schema_version VALUES (7);
";

/// Adds `tasks` (a todolist item) and links work sessions to one.
///
/// A "section" in the UI is just a subject - there is no separate sections
/// table. `tasks.subject_id` follows the exact same nullable / ON DELETE SET
/// NULL pattern as `sessions.subject_id` (MIGRATION_7): deleting a subject
/// does not delete its tasks, they fall back to the Uncategorised section.
///
/// `est_minutes` is nullable (no estimate given) and stored in minutes, not
/// pomodoro rounds, so it reads directly as "about 45 minutes" without a
/// conversion through `time_work_secs`.
const MIGRATION_8: &str = "
    CREATE TABLE IF NOT EXISTS tasks (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        title        TEXT NOT NULL,
        subject_id   INTEGER REFERENCES subjects(id) ON DELETE SET NULL,
        done         INTEGER NOT NULL DEFAULT 0 CHECK(done IN (0, 1)),
        est_minutes  INTEGER,
        created_at   INTEGER NOT NULL,
        completed_at INTEGER,
        sort_order   INTEGER NOT NULL DEFAULT 0
    );

    CREATE INDEX IF NOT EXISTS idx_tasks_subject ON tasks(subject_id);

    ALTER TABLE sessions ADD COLUMN task_id INTEGER
        REFERENCES tasks(id) ON DELETE SET NULL;

    CREATE INDEX IF NOT EXISTS idx_sessions_task ON sessions(task_id);

    INSERT INTO schema_version VALUES (8);
";

/// Counts focus that didn't finish the round (fork addition).
///
/// `focused_secs` is the time a work session adds to study totals and the week
/// calendar: the full length for a completed round, the seconds actually
/// focused for one that was skipped or reset part way (at least a minute), and
/// NULL when there is nothing to count. `completed` still decides what counts
/// as a pomodoro. Older unfinished rows never recorded how long they ran, so
/// only completed rows are backfilled.
///
/// `progress_secs` is a checkpoint of the round in progress, written every
/// minute, so a round cut short by quitting the app is still counted the next
/// time it opens (`queries::close_abandoned_sessions`).
const MIGRATION_9: &str = "
    ALTER TABLE sessions ADD COLUMN focused_secs INTEGER;
    ALTER TABLE sessions ADD COLUMN progress_secs INTEGER;

    UPDATE sessions SET focused_secs = duration_secs
     WHERE round_type = 'work' AND completed = 1 AND duration_secs > 0;

    INSERT INTO schema_version VALUES (9);
";

/// Adds `jots` (碎碎念): stray thoughts and to-dos jotted down mid-round.
///
/// `done_at` marks a jot crossed off (NULL = still open). `task_id` is set when
/// the jot was turned into a task; it follows the tasks FK pattern, so deleting
/// that task later only clears the link. `subject_id` and `in_focus` record
/// where the thought came up (the active subject, and whether a focus round
/// was under way) and are shown as context only.
const MIGRATION_10: &str = "
    CREATE TABLE IF NOT EXISTS jots (
        id         INTEGER PRIMARY KEY AUTOINCREMENT,
        body       TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        done_at    INTEGER,
        task_id    INTEGER REFERENCES tasks(id) ON DELETE SET NULL,
        subject_id INTEGER REFERENCES subjects(id) ON DELETE SET NULL,
        in_focus   INTEGER NOT NULL DEFAULT 0 CHECK(in_focus IN (0, 1))
    );

    CREATE INDEX IF NOT EXISTS idx_jots_done ON jots(done_at);

    INSERT INTO schema_version VALUES (10);
";

/// Apply any pending migrations. Each migration is wrapped in a transaction
/// so a partial failure leaves the database unchanged.
pub fn run(conn: &Connection) -> Result<()> {
    let version = current_version(conn)?;

    if version < 1 {
        log::info!("[db/migrations] applying MIGRATION_1: initial schema");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_1} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_1 complete");
    }

    if version < 2 {
        log::info!("[db/migrations] applying MIGRATION_2: timer durations minutes → seconds");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_2} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_2 complete");
    }

    if version < 3 {
        log::info!("[db/migrations] applying MIGRATION_3: seed check_for_updates setting");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_3} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_3 complete");
    }

    if version < 4 {
        log::info!("[db/migrations] applying MIGRATION_4: seed global_shortcuts_enabled setting");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_4} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_4 complete");
    }

    if version < 5 {
        log::info!("[db/migrations] applying MIGRATION_5: seed short_breaks_enabled and long_breaks_enabled");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_5} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_5 complete");
    }

    if version < 6 {
        log::info!("[db/migrations] applying MIGRATION_6: seed local shortcut key bindings");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_6} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_6 complete");
    }

    if version < 7 {
        log::info!("[db/migrations] applying MIGRATION_7: subjects table + sessions.subject_id");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_7} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_7 complete");
    }

    if version < 8 {
        log::info!("[db/migrations] applying MIGRATION_8: tasks table + sessions.task_id");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_8} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_8 complete");
    }

    if version < 9 {
        log::info!("[db/migrations] applying MIGRATION_9: sessions.focused_secs + progress_secs");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_9} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_9 complete");
    }

    if version < 10 {
        log::info!("[db/migrations] applying MIGRATION_10: jots table");
        conn.execute_batch(&format!("BEGIN; {MIGRATION_10} COMMIT;"))?;
        log::info!("[db/migrations] MIGRATION_10 complete");
    }

    Ok(())
}

/// Returns the current schema version, or 0 if the database is fresh.
fn current_version(conn: &Connection) -> Result<i64> {
    let table_exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='schema_version'",
        [],
        |row| row.get(0),
    )?;

    if !table_exists {
        return Ok(0);
    }

    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |row| row.get(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        // Second run must not error (version check prevents re-application).
        run(&conn).unwrap();
        let v: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 10);
    }

    #[test]
    fn all_tables_created() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        for table in &["settings", "sessions", "custom_themes", "schema_version", "subjects", "tasks", "jots"] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "table '{table}' was not created");
        }
    }

    #[test]
    fn sessions_has_nullable_subject_id() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        // The column exists, accepts NULL, and has no NOT NULL constraint.
        let notnull: i64 = conn
            .query_row(
                "SELECT \"notnull\" FROM pragma_table_info('sessions') WHERE name = 'subject_id'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(notnull, 0, "subject_id must be nullable");
    }

    #[test]
    fn deleting_a_subject_keeps_its_sessions() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run(&conn).unwrap();
        conn.execute(
            "INSERT INTO subjects (name, color, created_at) VALUES ('Math', '#ff0000', 0)",
            [],
        )
        .unwrap();
        let sid: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO sessions (started_at, round_type, duration_secs, completed, subject_id)
             VALUES (0, 'work', 1500, 1, ?1)",
            [sid],
        )
        .unwrap();

        conn.execute("DELETE FROM subjects WHERE id = ?1", [sid]).unwrap();

        // The session survives; only the link is cleared.
        let (count, subject): (i64, Option<i64>) = conn
            .query_row(
                "SELECT COUNT(*), MAX(subject_id) FROM sessions",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(count, 1, "deleting a subject must not delete its sessions");
        assert_eq!(subject, None, "subject_id should be set to NULL");
    }

    #[test]
    fn tasks_subject_id_is_nullable() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        let notnull: i64 = conn
            .query_row(
                "SELECT \"notnull\" FROM pragma_table_info('tasks') WHERE name = 'subject_id'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(notnull, 0, "tasks.subject_id must be nullable");
    }

    #[test]
    fn deleting_a_subject_keeps_its_tasks() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run(&conn).unwrap();
        conn.execute(
            "INSERT INTO subjects (name, color, created_at) VALUES ('Math', '#ff0000', 0)",
            [],
        )
        .unwrap();
        let sid: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tasks (title, subject_id, created_at) VALUES ('Ch. 7 problems', ?1, 0)",
            [sid],
        )
        .unwrap();

        conn.execute("DELETE FROM subjects WHERE id = ?1", [sid]).unwrap();

        let (count, subject): (i64, Option<i64>) = conn
            .query_row("SELECT COUNT(*), MAX(subject_id) FROM tasks", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(count, 1, "deleting a subject must not delete its tasks");
        assert_eq!(subject, None);
    }

    #[test]
    fn deleting_a_task_keeps_its_sessions() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run(&conn).unwrap();
        conn.execute(
            "INSERT INTO tasks (title, created_at) VALUES ('Read chapter 3', 0)",
            [],
        )
        .unwrap();
        let tid: i64 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO sessions (started_at, round_type, duration_secs, completed, task_id)
             VALUES (0, 'work', 1500, 1, ?1)",
            [tid],
        )
        .unwrap();

        conn.execute("DELETE FROM tasks WHERE id = ?1", [tid]).unwrap();

        let (count, task): (i64, Option<i64>) = conn
            .query_row("SELECT COUNT(*), MAX(task_id) FROM sessions", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(count, 1, "deleting a task must not delete its sessions");
        assert_eq!(task, None);
    }
}
