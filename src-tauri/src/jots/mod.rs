//! Jots (碎碎念): things that come to mind during a round — a to-do, an idea,
//! something to say — written down in a second so focus can carry on.
//!
//! A jot is open until it is crossed off (`done_at`) or turned into a task
//! (`task_id`, which also stamps `done_at`). Both keep the jot, so the pad can
//! show what was handled and undo it; "clear" deletes handled jots for good.
//!
//! Where a jot came up — the active subject, and whether a focus round was
//! under way — is recorded at creation and shown as context only. It is not
//! where a task made from it goes: most stray thoughts ("call mum") have
//! nothing to do with the subject being studied, so the caller picks.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::tasks::{self, Task, TaskError};

/// Longest accepted jot, in characters. Room for a few sentences; a jot is
/// not a diary.
pub const MAX_BODY_CHARS: usize = 500;
/// Longest task title (tasks::MAX_TITLE_CHARS); longer jots are cut with "…".
const MAX_TASK_TITLE_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Jot {
    pub id: i64,
    pub body: String,
    pub created_at: i64,
    /// When it was crossed off or turned into a task; `None` = still open.
    pub done_at: Option<i64>,
    /// The task it became, while that task exists.
    pub task_id: Option<i64>,
    /// Subject active when it was written (context only).
    pub subject_id: Option<i64>,
    /// Written while a focus round was running or paused.
    pub in_focus: bool,
}

/// Where a jot came up, captured by the caller from the timer and settings.
#[derive(Debug, Clone, Copy, Default)]
pub struct Context {
    pub subject_id: Option<i64>,
    pub in_focus: bool,
}

#[derive(Debug)]
pub enum JotError {
    Db(rusqlite::Error),
    Empty,
    TooLong(usize),
    NotFound(i64),
    /// Crossing off / converting a jot that was already turned into a task.
    AlreadyTask(i64),
    Task(TaskError),
}

impl std::fmt::Display for JotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JotError::Db(e) => write!(f, "database error: {e}"),
            JotError::Empty => write!(f, "a jot cannot be empty"),
            JotError::TooLong(n) => {
                write!(f, "jot is {n} characters, maximum is {MAX_BODY_CHARS}")
            }
            JotError::NotFound(id) => write!(f, "no jot with id {id}"),
            JotError::AlreadyTask(id) => write!(f, "jot {id} is already a task"),
            JotError::Task(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for JotError {}

impl From<rusqlite::Error> for JotError {
    fn from(e: rusqlite::Error) -> Self {
        JotError::Db(e)
    }
}

impl From<TaskError> for JotError {
    fn from(e: TaskError) -> Self {
        match e {
            TaskError::Db(e) => JotError::Db(e),
            other => JotError::Task(other),
        }
    }
}

pub type Result<T> = std::result::Result<T, JotError>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Trims the ends and normalises line breaks; inner blank lines are kept.
fn clean_body(body: &str) -> Result<String> {
    let trimmed = body.replace("\r\n", "\n").trim().to_string();
    if trimmed.is_empty() {
        return Err(JotError::Empty);
    }
    let len = trimmed.chars().count();
    if len > MAX_BODY_CHARS {
        return Err(JotError::TooLong(len));
    }
    Ok(trimmed)
}

/// One line, at most a task title long.
pub fn task_title(body: &str) -> String {
    let line = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= MAX_TASK_TITLE_CHARS {
        return line;
    }
    let cut: String = line.chars().take(MAX_TASK_TITLE_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

const SELECT: &str =
    "SELECT id, body, created_at, done_at, task_id, subject_id, in_focus FROM jots";

fn row_to_jot(row: &rusqlite::Row<'_>) -> rusqlite::Result<Jot> {
    Ok(Jot {
        id: row.get(0)?,
        body: row.get(1)?,
        created_at: row.get(2)?,
        done_at: row.get(3)?,
        task_id: row.get(4)?,
        subject_id: row.get(5)?,
        in_focus: row.get::<_, i64>(6)? != 0,
    })
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

/// Every jot: open ones newest first, then handled ones, most recently handled first.
pub fn list(conn: &Connection) -> Result<Vec<Jot>> {
    let sql = format!(
        "{SELECT} ORDER BY done_at IS NOT NULL, done_at DESC, created_at DESC, id DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_jot)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get(conn: &Connection, id: i64) -> Result<Jot> {
    conn.query_row(&format!("{SELECT} WHERE id = ?1"), params![id], row_to_jot)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => JotError::NotFound(id),
            other => JotError::Db(other),
        })
}

pub fn create(conn: &Connection, body: &str, context: Context) -> Result<Jot> {
    let body = clean_body(body)?;
    // A subject deleted a moment ago must not fail the jot; drop the context instead.
    let subject_id: Option<i64> = match context.subject_id {
        Some(id) => conn
            .query_row("SELECT id FROM subjects WHERE id = ?1", params![id], |r| r.get(0))
            .ok(),
        None => None,
    };
    conn.execute(
        "INSERT INTO jots (body, created_at, subject_id, in_focus) VALUES (?1, ?2, ?3, ?4)",
        params![body, unix_now(), subject_id, context.in_focus as i64],
    )?;
    let id = conn.last_insert_rowid();
    log::info!("[jots] created id={id} in_focus={}", context.in_focus);
    get(conn, id)
}

pub fn edit(conn: &Connection, id: i64, body: &str) -> Result<Jot> {
    let body = clean_body(body)?;
    let n = conn.execute("UPDATE jots SET body = ?1 WHERE id = ?2", params![body, id])?;
    if n == 0 {
        return Err(JotError::NotFound(id));
    }
    get(conn, id)
}

/// Cross off (or bring back) a jot. A jot that became a task is undone with
/// [`untask`] instead, so the task is not left behind.
pub fn set_done(conn: &Connection, id: i64, done: bool) -> Result<Jot> {
    let jot = get(conn, id)?;
    if jot.task_id.is_some() {
        return Err(JotError::AlreadyTask(id));
    }
    let done_at = if done { Some(jot.done_at.unwrap_or_else(unix_now)) } else { None };
    conn.execute("UPDATE jots SET done_at = ?1 WHERE id = ?2", params![done_at, id])?;
    get(conn, id)
}

/// Turn a jot into a task in `subject_id`'s section (`None` = Uncategorised).
/// The jot is kept, handled, and linked to the new task.
pub fn to_task(conn: &mut Connection, id: i64, subject_id: Option<i64>) -> Result<(Jot, Task)> {
    let tx = conn.transaction()?;
    let jot = get(&tx, id)?;
    if jot.task_id.is_some() {
        return Err(JotError::AlreadyTask(id));
    }
    let task = tasks::create(&tx, &task_title(&jot.body), subject_id, None)?;
    tx.execute(
        "UPDATE jots SET task_id = ?1, done_at = ?2 WHERE id = ?3",
        params![task.id, unix_now(), id],
    )?;
    let jot = get(&tx, id)?;
    tx.commit()?;
    log::info!("[jots] id={id} became task id={}", task.id);
    Ok((jot, task))
}

/// Undo [`to_task`]: delete the task it made (if it is still there — time
/// already logged to it is kept, as with any task deletion) and reopen the jot.
/// Returns the reopened jot and the id of the task that was removed.
pub fn untask(conn: &mut Connection, id: i64) -> Result<(Jot, Option<i64>)> {
    let tx = conn.transaction()?;
    let jot = get(&tx, id)?;
    let removed = match jot.task_id {
        Some(task_id) => match tasks::delete(&tx, task_id) {
            Ok(()) => Some(task_id),
            Err(TaskError::NotFound(_)) => None,
            Err(e) => return Err(e.into()),
        },
        None => None,
    };
    tx.execute("UPDATE jots SET task_id = NULL, done_at = NULL WHERE id = ?1", params![id])?;
    let jot = get(&tx, id)?;
    tx.commit()?;
    Ok((jot, removed))
}

/// Delete a jot; returns it so it can be put back with [`restore`].
pub fn delete(conn: &Connection, id: i64) -> Result<Jot> {
    let jot = get(conn, id)?;
    conn.execute("DELETE FROM jots WHERE id = ?1", params![id])?;
    Ok(jot)
}

/// Put a deleted jot back exactly as it was (same id, times and links). Links
/// to a task or subject deleted in the meantime are dropped.
pub fn restore(conn: &Connection, jot: &Jot) -> Result<Jot> {
    let body = clean_body(&jot.body)?;
    let exists = |table: &str, id: Option<i64>| -> Option<i64> {
        let id = id?;
        conn.query_row(&format!("SELECT id FROM {table} WHERE id = ?1"), params![id], |r| r.get(0))
            .ok()
    };
    let task_id = exists("tasks", jot.task_id);
    let subject_id = exists("subjects", jot.subject_id);
    conn.execute(
        "INSERT INTO jots (id, body, created_at, done_at, task_id, subject_id, in_focus)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![jot.id, body, jot.created_at, jot.done_at, task_id, subject_id, jot.in_focus as i64],
    )?;
    get(conn, jot.id)
}

/// Delete every handled jot (crossed off or turned into a task). Tasks made
/// from them are not touched. Returns how many were removed.
pub fn clear_handled(conn: &Connection) -> Result<usize> {
    Ok(conn.execute("DELETE FROM jots WHERE done_at IS NOT NULL", [])?)
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
            "INSERT INTO subjects (name, color, created_at) VALUES (?1, '#C83224', 0)",
            params![name],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn focus_in(subject_id: Option<i64>) -> Context {
        Context { subject_id, in_focus: true }
    }

    #[test]
    fn create_trims_keeps_inner_lines_and_records_context() {
        let conn = setup();
        let math = make_subject(&conn, "Math");
        let j = create(&conn, "  call mum\r\n\r\nabout Sunday  ", focus_in(Some(math))).unwrap();
        assert_eq!(j.body, "call mum\n\nabout Sunday");
        assert_eq!(j.subject_id, Some(math));
        assert!(j.in_focus);
        assert_eq!(j.done_at, None);
        assert_eq!(j.task_id, None);
    }

    #[test]
    fn body_is_required_and_bounded() {
        let conn = setup();
        assert!(matches!(create(&conn, " \n ", Context::default()), Err(JotError::Empty)));
        let long = "念".repeat(MAX_BODY_CHARS + 1);
        assert!(matches!(create(&conn, &long, Context::default()), Err(JotError::TooLong(501))));
        assert!(create(&conn, &"念".repeat(MAX_BODY_CHARS), Context::default()).is_ok());
    }

    #[test]
    fn a_missing_subject_is_dropped_not_an_error() {
        let conn = setup();
        let j = create(&conn, "idea", focus_in(Some(999))).unwrap();
        assert_eq!(j.subject_id, None);
    }

    #[test]
    fn list_puts_open_jots_first_newest_first() {
        let conn = setup();
        let a = create(&conn, "a", Context::default()).unwrap();
        let b = create(&conn, "b", Context::default()).unwrap();
        let c = create(&conn, "c", Context::default()).unwrap();
        // Same second in a fast test: give them distinct times.
        for (id, t) in [(a.id, 100), (b.id, 200), (c.id, 300)] {
            conn.execute("UPDATE jots SET created_at = ?1 WHERE id = ?2", params![t, id]).unwrap();
        }
        set_done(&conn, c.id, true).unwrap();
        let ids: Vec<i64> = list(&conn).unwrap().iter().map(|j| j.id).collect();
        assert_eq!(ids, vec![b.id, a.id, c.id]);
    }

    #[test]
    fn crossing_off_stamps_and_bringing_back_clears() {
        let conn = setup();
        let j = create(&conn, "x", Context::default()).unwrap();
        let done = set_done(&conn, j.id, true).unwrap();
        assert!(done.done_at.is_some());
        // Crossing off twice keeps the first time.
        conn.execute("UPDATE jots SET done_at = 5 WHERE id = ?1", params![j.id]).unwrap();
        assert_eq!(set_done(&conn, j.id, true).unwrap().done_at, Some(5));
        assert_eq!(set_done(&conn, j.id, false).unwrap().done_at, None);
    }

    #[test]
    fn edit_replaces_the_text() {
        let conn = setup();
        let j = create(&conn, "buy milk", Context::default()).unwrap();
        assert_eq!(edit(&conn, j.id, " buy oat milk ").unwrap().body, "buy oat milk");
        assert!(matches!(edit(&conn, j.id, ""), Err(JotError::Empty)));
        assert!(matches!(edit(&conn, 999, "x"), Err(JotError::NotFound(999))));
    }

    #[test]
    fn to_task_makes_a_one_line_task_where_asked() {
        let mut conn = setup();
        let math = make_subject(&conn, "Math");
        let english = make_subject(&conn, "English");
        let j = create(&conn, "try Lagrange\non problem 7", focus_in(Some(math))).unwrap();

        let (jot, task) = to_task(&mut conn, j.id, Some(english)).unwrap();
        assert_eq!(task.title, "try Lagrange on problem 7");
        assert_eq!(task.subject_id, Some(english), "the chosen section, not the jot's context");
        assert_eq!(jot.task_id, Some(task.id));
        assert!(jot.done_at.is_some());

        // Converting twice, or crossing off a converted jot, is refused.
        assert!(matches!(to_task(&mut conn, j.id, None), Err(JotError::AlreadyTask(_))));
        assert!(matches!(set_done(&conn, j.id, false), Err(JotError::AlreadyTask(_))));
    }

    #[test]
    fn to_task_into_a_missing_section_changes_nothing() {
        let mut conn = setup();
        let j = create(&conn, "x", Context::default()).unwrap();
        assert!(matches!(to_task(&mut conn, j.id, Some(999)), Err(JotError::Task(_))));
        assert_eq!(get(&conn, j.id).unwrap(), j);
        assert!(tasks::list(&conn, true).unwrap().is_empty());
    }

    #[test]
    fn long_jots_become_titles_that_fit() {
        let title = task_title(&"a".repeat(600));
        assert_eq!(title.chars().count(), MAX_TASK_TITLE_CHARS);
        assert!(title.ends_with('…'));
        assert_eq!(task_title("  one\n two  three "), "one two three");
    }

    #[test]
    fn untask_removes_the_task_and_reopens_the_jot() {
        let mut conn = setup();
        let j = create(&conn, "plan next week", Context::default()).unwrap();
        let (_, task) = to_task(&mut conn, j.id, None).unwrap();

        let (jot, removed) = untask(&mut conn, j.id).unwrap();
        assert_eq!(removed, Some(task.id));
        assert_eq!(jot.task_id, None);
        assert_eq!(jot.done_at, None);
        assert!(tasks::list(&conn, true).unwrap().is_empty());
    }

    #[test]
    fn deleting_the_task_later_only_clears_the_link() {
        let mut conn = setup();
        let j = create(&conn, "x", Context::default()).unwrap();
        let (_, task) = to_task(&mut conn, j.id, None).unwrap();
        tasks::delete(&conn, task.id).unwrap();
        let jot = get(&conn, j.id).unwrap();
        assert_eq!(jot.task_id, None);
        assert!(jot.done_at.is_some(), "still handled");
        // And untask then just reopens it.
        let (jot, removed) = untask(&mut conn, j.id).unwrap();
        assert_eq!(removed, None);
        assert_eq!(jot.done_at, None);
    }

    #[test]
    fn delete_then_restore_round_trips() {
        let conn = setup();
        let math = make_subject(&conn, "Math");
        let j = create(&conn, "idea", focus_in(Some(math))).unwrap();
        let gone = delete(&conn, j.id).unwrap();
        assert!(matches!(get(&conn, j.id), Err(JotError::NotFound(_))));
        assert_eq!(restore(&conn, &gone).unwrap(), j);

        // A subject deleted while the jot was gone is dropped on restore.
        let gone = delete(&conn, j.id).unwrap();
        conn.execute("DELETE FROM subjects WHERE id = ?1", params![math]).unwrap();
        assert_eq!(restore(&conn, &gone).unwrap().subject_id, None);
    }

    #[test]
    fn clear_handled_keeps_open_jots_and_tasks() {
        let mut conn = setup();
        let open = create(&conn, "open", Context::default()).unwrap();
        let done = create(&conn, "done", Context::default()).unwrap();
        let tasked = create(&conn, "tasked", Context::default()).unwrap();
        set_done(&conn, done.id, true).unwrap();
        to_task(&mut conn, tasked.id, None).unwrap();

        assert_eq!(clear_handled(&conn).unwrap(), 2);
        let left: Vec<i64> = list(&conn).unwrap().iter().map(|j| j.id).collect();
        assert_eq!(left, vec![open.id]);
        assert_eq!(tasks::list(&conn, true).unwrap().len(), 1, "the task stays");
    }

    #[test]
    fn deleting_a_subject_keeps_its_jots() {
        let conn = setup();
        let math = make_subject(&conn, "Math");
        let j = create(&conn, "x", focus_in(Some(math))).unwrap();
        conn.execute("DELETE FROM subjects WHERE id = ?1", params![math]).unwrap();
        assert_eq!(get(&conn, j.id).unwrap().subject_id, None);
    }
}
