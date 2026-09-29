//! Task (todolist item) CRUD.
//!
//! A task belongs to a subject — the UI groups tasks into one "section" per
//! subject, but that grouping is purely client-side: there is no sections
//! table. `subject_id` is nullable and `ON DELETE SET NULL` (see MIGRATION_8),
//! mirroring `sessions.subject_id` from the subjects module: deleting a
//! subject falls its tasks back into the Uncategorised section rather than
//! deleting them.
//!
//! Deleting a *task* keeps any sessions logged against it (same FK pattern),
//! so time already spent on a task survives its removal from the list.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// Longest accepted task title, in characters (not bytes). Generous relative
/// to a subject name — a todo title is often a full sentence.
const MAX_TITLE_CHARS: usize = 200;
/// Longest accepted estimate, in minutes (24 hours). Rejects fat-fingered
/// entry like "450" meant as seconds or a stray extra digit.
const MAX_EST_MINUTES: u32 = 1440;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub subject_id: Option<i64>,
    pub done: bool,
    pub est_minutes: Option<u32>,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    pub sort_order: i64,
    /// Seconds focused in work sessions logged against this task, unfinished
    /// rounds included. Computed on read, not stored — lets "estimate vs actual"
    /// stay correct even as new sessions land without a separate update.
    pub actual_secs: i64,
}

#[derive(Debug)]
pub enum TaskError {
    Db(rusqlite::Error),
    EmptyTitle,
    TitleTooLong(usize),
    InvalidEstimate(u32),
    SubjectNotFound(i64),
    NotFound(i64),
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::Db(e) => write!(f, "database error: {e}"),
            TaskError::EmptyTitle => write!(f, "task title cannot be empty"),
            TaskError::TitleTooLong(n) => {
                write!(f, "task title is {n} characters, maximum is {MAX_TITLE_CHARS}")
            }
            TaskError::InvalidEstimate(m) => {
                write!(f, "{m} minutes is not a valid estimate (1\u{2013}{MAX_EST_MINUTES})")
            }
            TaskError::SubjectNotFound(id) => write!(f, "no subject with id {id}"),
            TaskError::NotFound(id) => write!(f, "no task with id {id}"),
        }
    }
}

impl std::error::Error for TaskError {}

impl From<rusqlite::Error> for TaskError {
    fn from(e: rusqlite::Error) -> Self {
        TaskError::Db(e)
    }
}

pub type Result<T> = std::result::Result<T, TaskError>;

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn clean_title(title: &str) -> Result<String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err(TaskError::EmptyTitle);
    }
    let len = trimmed.chars().count();
    if len > MAX_TITLE_CHARS {
        return Err(TaskError::TitleTooLong(len));
    }
    Ok(trimmed.to_string())
}

fn check_estimate(est_minutes: Option<u32>) -> Result<()> {
    match est_minutes {
        Some(m) if m == 0 || m > MAX_EST_MINUTES => Err(TaskError::InvalidEstimate(m)),
        _ => Ok(()),
    }
}

fn check_subject_exists(conn: &Connection, subject_id: Option<i64>) -> Result<()> {
    let Some(id) = subject_id else { return Ok(()) };
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM subjects WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(TaskError::SubjectNotFound(id))
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn row_to_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        subject_id: row.get(2)?,
        done: row.get::<_, i64>(3)? != 0,
        est_minutes: row.get(4)?,
        created_at: row.get(5)?,
        completed_at: row.get(6)?,
        sort_order: row.get(7)?,
        actual_secs: row.get(8)?,
    })
}

const SELECT_BASE: &str = "
    SELECT t.id, t.title, t.subject_id, t.done, t.est_minutes, t.created_at,
           t.completed_at, t.sort_order,
           COALESCE(SUM(
               CASE WHEN s.round_type = 'work' THEN s.focused_secs END
           ), 0) AS actual_secs
    FROM tasks t
    LEFT JOIN sessions s ON s.task_id = t.id";

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

/// All tasks, ordered so tasks sharing a subject stay contiguous with a
/// stable in-section order. The frontend buckets the flat list by
/// `subject_id` itself (it already has the subject list loaded), so
/// cross-subject interleaving here does not matter.
pub fn list(conn: &Connection, include_done: bool) -> Result<Vec<Task>> {
    let where_clause = if include_done { "" } else { "WHERE t.done = 0" };
    let sql = format!(
        "{SELECT_BASE}
         {where_clause}
         GROUP BY t.id
         ORDER BY t.subject_id IS NULL, t.subject_id, t.sort_order, t.created_at, t.id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_task)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get(conn: &Connection, id: i64) -> Result<Task> {
    let sql = format!("{SELECT_BASE} WHERE t.id = ?1 GROUP BY t.id");
    conn.query_row(&sql, params![id], row_to_task).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => TaskError::NotFound(id),
        other => TaskError::Db(other),
    })
}

/// Create a task. New tasks sort to the end of their section.
pub fn create(
    conn: &Connection,
    title: &str,
    subject_id: Option<i64>,
    est_minutes: Option<u32>,
) -> Result<Task> {
    let title = clean_title(title)?;
    check_estimate(est_minutes)?;
    check_subject_exists(conn, subject_id)?;

    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM tasks
         WHERE subject_id IS ?1",
        params![subject_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO tasks (title, subject_id, done, est_minutes, created_at, sort_order)
         VALUES (?1, ?2, 0, ?3, ?4, ?5)",
        params![title, subject_id, est_minutes, unix_now(), next_order],
    )?;
    let id = conn.last_insert_rowid();
    log::info!("[tasks] created id={id} subject={subject_id:?} title='{title}'");
    get(conn, id)
}

pub fn rename(conn: &Connection, id: i64, title: &str) -> Result<Task> {
    let title = clean_title(title)?;
    let n = conn.execute("UPDATE tasks SET title = ?1 WHERE id = ?2", params![title, id])?;
    if n == 0 {
        return Err(TaskError::NotFound(id));
    }
    get(conn, id)
}

/// `None` clears the estimate.
pub fn set_estimate(conn: &Connection, id: i64, est_minutes: Option<u32>) -> Result<Task> {
    check_estimate(est_minutes)?;
    let n = conn.execute(
        "UPDATE tasks SET est_minutes = ?1 WHERE id = ?2",
        params![est_minutes, id],
    )?;
    if n == 0 {
        return Err(TaskError::NotFound(id));
    }
    get(conn, id)
}

/// Move a task to a different section. `None` moves it to Uncategorised.
/// Resets `sort_order` to the end of the destination section, same as a
/// freshly created task there.
pub fn move_to_subject(conn: &Connection, id: i64, subject_id: Option<i64>) -> Result<Task> {
    check_subject_exists(conn, subject_id)?;
    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM tasks WHERE subject_id IS ?1",
        params![subject_id],
        |r| r.get(0),
    )?;
    let n = conn.execute(
        "UPDATE tasks SET subject_id = ?1, sort_order = ?2 WHERE id = ?3",
        params![subject_id, next_order, id],
    )?;
    if n == 0 {
        return Err(TaskError::NotFound(id));
    }
    get(conn, id)
}

pub fn set_done(conn: &Connection, id: i64, done: bool) -> Result<Task> {
    let completed_at = if done { Some(unix_now()) } else { None };
    let n = conn.execute(
        "UPDATE tasks SET done = ?1, completed_at = ?2 WHERE id = ?3",
        params![done as i64, completed_at, id],
    )?;
    if n == 0 {
        return Err(TaskError::NotFound(id));
    }
    get(conn, id)
}

/// Delete a task. Sessions logged against it are kept — the FK clears
/// `task_id`, so time already spent on it is not lost.
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let n = conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(TaskError::NotFound(id));
    }
    log::info!("[tasks] deleted id={id} (its sessions were kept)");
    Ok(())
}

/// Persist a new display order within one section. `ids` is that section's
/// full task list, in the desired order; ids that do not belong to
/// `subject_id` are skipped rather than silently reassigning them.
pub fn reorder(conn: &mut Connection, subject_id: Option<i64>, ids: &[i64]) -> Result<()> {
    let tx = conn.transaction()?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE tasks SET sort_order = ?1 WHERE id = ?2 AND subject_id IS ?3",
            params![i as i64, id, subject_id],
        )?;
    }
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        migrations::run(&conn).unwrap();
        conn
    }

    fn make_subject(conn: &Connection, name: &str) -> i64 {
        conn.execute(
            "INSERT INTO subjects (name, color, created_at) VALUES (?1, '#61afef', 0)",
            params![name],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn create_and_list_defaults_to_uncategorised() {
        let conn = setup();
        let t = create(&conn, "Read chapter 3", None, Some(30)).unwrap();
        assert_eq!(t.subject_id, None);
        assert_eq!(t.est_minutes, Some(30));
        assert!(!t.done);
        assert_eq!(t.actual_secs, 0);
        assert_eq!(t.sort_order, 0);

        let all = list(&conn, false).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, t.id);
    }

    #[test]
    fn title_is_trimmed_and_required() {
        let conn = setup();
        let t = create(&conn, "  Finish essay  ", None, None).unwrap();
        assert_eq!(t.title, "Finish essay");
        assert!(matches!(create(&conn, "   ", None, None), Err(TaskError::EmptyTitle)));
    }

    #[test]
    fn estimate_bounds_are_enforced() {
        let conn = setup();
        assert!(matches!(
            create(&conn, "X", None, Some(0)),
            Err(TaskError::InvalidEstimate(0))
        ));
        assert!(matches!(
            create(&conn, "X", None, Some(1441)),
            Err(TaskError::InvalidEstimate(1441))
        ));
        let t = create(&conn, "X", None, Some(1440)).unwrap();
        assert_eq!(t.est_minutes, Some(1440));
    }

    #[test]
    fn creating_with_unknown_subject_is_rejected() {
        let conn = setup();
        assert!(matches!(
            create(&conn, "X", Some(999), None),
            Err(TaskError::SubjectNotFound(999))
        ));
    }

    #[test]
    fn sort_order_is_scoped_per_section() {
        let conn = setup();
        let math = make_subject(&conn, "Math");
        let a = create(&conn, "A", Some(math), None).unwrap();
        let b = create(&conn, "B", Some(math), None).unwrap();
        // Uncategorised has its own independent counter, starting at 0 again.
        let c = create(&conn, "C", None, None).unwrap();
        assert_eq!(a.sort_order, 0);
        assert_eq!(b.sort_order, 1);
        assert_eq!(c.sort_order, 0);
    }

    #[test]
    fn set_done_stamps_and_clears_completed_at() {
        let conn = setup();
        let t = create(&conn, "X", None, None).unwrap();
        let done = set_done(&conn, t.id, true).unwrap();
        assert!(done.done);
        assert!(done.completed_at.is_some());

        let undone = set_done(&conn, t.id, false).unwrap();
        assert!(!undone.done);
        assert_eq!(undone.completed_at, None);
    }

    #[test]
    fn list_excludes_done_by_default() {
        let conn = setup();
        let a = create(&conn, "A", None, None).unwrap();
        create(&conn, "B", None, None).unwrap();
        set_done(&conn, a.id, true).unwrap();

        assert_eq!(list(&conn, false).unwrap().len(), 1);
        assert_eq!(list(&conn, true).unwrap().len(), 2);
    }

    #[test]
    fn move_to_subject_changes_section_and_resets_order() {
        let conn = setup();
        let math = make_subject(&conn, "Math");
        let hist = make_subject(&conn, "History");
        create(&conn, "existing in history", Some(hist), None).unwrap();
        let t = create(&conn, "moving task", Some(math), None).unwrap();

        let moved = move_to_subject(&conn, t.id, Some(hist)).unwrap();
        assert_eq!(moved.subject_id, Some(hist));
        // Appended after the task already in History (sort_order 0).
        assert_eq!(moved.sort_order, 1);

        let uncategorised = move_to_subject(&conn, t.id, None).unwrap();
        assert_eq!(uncategorised.subject_id, None);
    }

    #[test]
    fn move_to_unknown_subject_is_rejected() {
        let conn = setup();
        let t = create(&conn, "X", None, None).unwrap();
        assert!(matches!(
            move_to_subject(&conn, t.id, Some(999)),
            Err(TaskError::SubjectNotFound(999))
        ));
    }

    #[test]
    fn deleting_a_subject_keeps_its_tasks_uncategorised() {
        let conn = setup();
        let math = make_subject(&conn, "Math");
        let t = create(&conn, "X", Some(math), None).unwrap();

        conn.execute("DELETE FROM subjects WHERE id = ?1", params![math]).unwrap();

        let reloaded = get(&conn, t.id).unwrap();
        assert_eq!(reloaded.subject_id, None);
    }

    #[test]
    fn deleting_a_task_keeps_its_logged_sessions() {
        let conn = setup();
        let t = create(&conn, "X", None, None).unwrap();
        crate::db::queries::insert_session(&conn, "work", 1500, None, None).unwrap();
        conn.execute(
            "UPDATE sessions SET task_id = ?1 WHERE task_id IS NULL",
            params![t.id],
        )
        .unwrap();

        delete(&conn, t.id).unwrap();

        let (count, task): (i64, Option<i64>) = conn
            .query_row("SELECT COUNT(*), MAX(task_id) FROM sessions", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(task, None);
    }

    #[test]
    fn actual_secs_sums_focus_from_work_sessions() {
        let conn = setup();
        let t = create(&conn, "X", None, None).unwrap();

        let id1 = crate::db::queries::insert_session(&conn, "work", 1500, None, None).unwrap();
        crate::db::queries::complete_session(&conn, id1, true, 1500).unwrap();
        conn.execute("UPDATE sessions SET task_id = ?1 WHERE id = ?2", params![t.id, id1])
            .unwrap();

        // Skipped 10 minutes in: those 10 minutes count.
        let id2 = crate::db::queries::insert_session(&conn, "work", 1500, None, None).unwrap();
        crate::db::queries::complete_session(&conn, id2, false, 600).unwrap();
        conn.execute("UPDATE sessions SET task_id = ?1 WHERE id = ?2", params![t.id, id2])
            .unwrap();

        // Break round: must not count even if somehow tagged.
        let id3 = crate::db::queries::insert_session(&conn, "short-break", 300, None, None).unwrap();
        crate::db::queries::complete_session(&conn, id3, true, 300).unwrap();
        conn.execute("UPDATE sessions SET task_id = ?1 WHERE id = ?2", params![t.id, id3])
            .unwrap();

        let reloaded = get(&conn, t.id).unwrap();
        assert_eq!(reloaded.actual_secs, 2100);
    }

    #[test]
    fn reorder_is_scoped_to_its_own_section() {
        let mut conn = setup();
        let math = make_subject(&conn, "Math");
        let a = create(&conn, "A", Some(math), None).unwrap();
        let b = create(&conn, "B", Some(math), None).unwrap();
        let c = create(&conn, "C", None, None).unwrap(); // different section

        // Reordering Math must not touch C's sort_order, even if its id were
        // (accidentally) included — it belongs to a different subject_id.
        reorder(&mut conn, Some(math), &[b.id, a.id, c.id]).unwrap();

        let reloaded_c = get(&conn, c.id).unwrap();
        assert_eq!(reloaded_c.sort_order, 0, "task outside the section must be untouched");

        let mut math_tasks: Vec<Task> = list(&conn, false)
            .unwrap()
            .into_iter()
            .filter(|t| t.subject_id == Some(math))
            .collect();
        math_tasks.sort_by_key(|t| t.sort_order);
        // B was moved first in the reorder call, so it now sorts before A.
        assert_eq!(math_tasks[0].id, b.id);
        assert_eq!(math_tasks[1].id, a.id);
    }
}
