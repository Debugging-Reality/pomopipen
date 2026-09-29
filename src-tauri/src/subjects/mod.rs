//! Subject CRUD.
//!
//! A subject is what a work round is attributed to — a school subject, a
//! course, a project. Sessions reference one via `sessions.subject_id`, which
//! is nullable: rounds started with no subject selected stay uncategorised.
//!
//! Deleting a subject is deliberately non-destructive for history — the
//! foreign key is `ON DELETE SET NULL`, so the sessions survive and fall back
//! into the uncategorised bucket. Archiving is the gentler option and is what
//! the UI should steer people towards.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// Longest accepted subject name, in characters (not bytes).
const MAX_NAME_CHARS: usize = 40;

/// Palette offered when creating a subject. Mid-saturation tones that stay
/// legible on both light and dark themes.
pub const PALETTE: &[&str] = &[
    "#e06c75", "#e59f4a", "#e5c07b", "#98c379", "#56b6c2", "#61afef", "#7e8ce0", "#c678dd",
    "#d47ba8", "#8b9bb4",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subject {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub archived: bool,
    pub created_at: i64,
    pub sort_order: i64,
}

#[derive(Debug)]
pub enum SubjectError {
    Db(rusqlite::Error),
    EmptyName,
    NameTooLong(usize),
    DuplicateName(String),
    InvalidColor(String),
    NotFound(i64),
}

impl std::fmt::Display for SubjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubjectError::Db(e) => write!(f, "database error: {e}"),
            SubjectError::EmptyName => write!(f, "subject name cannot be empty"),
            SubjectError::NameTooLong(n) => {
                write!(f, "subject name is {n} characters, maximum is {MAX_NAME_CHARS}")
            }
            SubjectError::DuplicateName(n) => write!(f, "a subject named '{n}' already exists"),
            SubjectError::InvalidColor(c) => {
                write!(f, "'{c}' is not a #rrggbb colour")
            }
            SubjectError::NotFound(id) => write!(f, "no subject with id {id}"),
        }
    }
}

impl std::error::Error for SubjectError {}

impl From<rusqlite::Error> for SubjectError {
    fn from(e: rusqlite::Error) -> Self {
        SubjectError::Db(e)
    }
}

pub type Result<T> = std::result::Result<T, SubjectError>;

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Trim and validate a subject name.
fn clean_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(SubjectError::EmptyName);
    }
    let len = trimmed.chars().count();
    if len > MAX_NAME_CHARS {
        return Err(SubjectError::NameTooLong(len));
    }
    Ok(trimmed.to_string())
}

/// Accept only `#rrggbb`. Normalised to lowercase so equality checks and CSS
/// output stay consistent.
fn clean_color(color: &str) -> Result<String> {
    let c = color.trim();
    let valid = c.len() == 7
        && c.starts_with('#')
        && c[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    if !valid {
        return Err(SubjectError::InvalidColor(c.to_string()));
    }
    Ok(c.to_lowercase())
}

/// Map SQLite's UNIQUE violation onto a friendlier error.
fn map_unique(e: rusqlite::Error, name: &str) -> SubjectError {
    let msg = e.to_string();
    if msg.contains("UNIQUE constraint failed") {
        SubjectError::DuplicateName(name.to_string())
    } else {
        SubjectError::Db(e)
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn row_to_subject(row: &rusqlite::Row<'_>) -> rusqlite::Result<Subject> {
    Ok(Subject {
        id: row.get(0)?,
        name: row.get(1)?,
        color: row.get(2)?,
        archived: row.get::<_, i64>(3)? != 0,
        created_at: row.get(4)?,
        sort_order: row.get(5)?,
    })
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

/// All subjects, ordered by `sort_order` then creation time.
pub fn list(conn: &Connection, include_archived: bool) -> Result<Vec<Subject>> {
    let sql = if include_archived {
        "SELECT id, name, color, archived, created_at, sort_order
         FROM subjects ORDER BY sort_order, created_at, id"
    } else {
        "SELECT id, name, color, archived, created_at, sort_order
         FROM subjects WHERE archived = 0 ORDER BY sort_order, created_at, id"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], row_to_subject)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get(conn: &Connection, id: i64) -> Result<Subject> {
    conn.query_row(
        "SELECT id, name, color, archived, created_at, sort_order FROM subjects WHERE id = ?1",
        params![id],
        row_to_subject,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => SubjectError::NotFound(id),
        other => SubjectError::Db(other),
    })
}

/// Create a subject. New subjects sort to the end of the list.
pub fn create(conn: &Connection, name: &str, color: &str) -> Result<Subject> {
    let name = clean_name(name)?;
    let color = clean_color(color)?;
    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM subjects",
        [],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO subjects (name, color, archived, created_at, sort_order)
         VALUES (?1, ?2, 0, ?3, ?4)",
        params![name, color, unix_now(), next_order],
    )
    .map_err(|e| map_unique(e, &name))?;
    let id = conn.last_insert_rowid();
    log::info!("[subjects] created id={id} name='{name}'");
    get(conn, id)
}

/// Rename and/or recolour. Both fields are optional so the UI can patch one.
pub fn update(
    conn: &Connection,
    id: i64,
    name: Option<&str>,
    color: Option<&str>,
) -> Result<Subject> {
    // Confirm it exists first so a no-op patch still reports NotFound.
    get(conn, id)?;

    if let Some(n) = name {
        let n = clean_name(n)?;
        conn.execute(
            "UPDATE subjects SET name = ?1 WHERE id = ?2",
            params![n, id],
        )
        .map_err(|e| map_unique(e, &n))?;
    }
    if let Some(c) = color {
        let c = clean_color(c)?;
        conn.execute(
            "UPDATE subjects SET color = ?1 WHERE id = ?2",
            params![c, id],
        )?;
    }
    get(conn, id)
}

pub fn set_archived(conn: &Connection, id: i64, archived: bool) -> Result<Subject> {
    let n = conn.execute(
        "UPDATE subjects SET archived = ?1 WHERE id = ?2",
        params![archived as i64, id],
    )?;
    if n == 0 {
        return Err(SubjectError::NotFound(id));
    }
    log::info!("[subjects] id={id} archived={archived}");
    get(conn, id)
}

/// Delete a subject. Its sessions are kept — the FK clears `subject_id`,
/// so that study time reappears in the uncategorised bucket.
///
/// Requires `PRAGMA foreign_keys=ON` (set when the connection is opened).
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let n = conn.execute("DELETE FROM subjects WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(SubjectError::NotFound(id));
    }
    log::info!("[subjects] deleted id={id} (its sessions were kept)");
    Ok(())
}

/// Persist a new display order. `ids` is the full list, in the desired order;
/// ids that do not exist are skipped.
pub fn reorder(conn: &mut Connection, ids: &[i64]) -> Result<()> {
    let tx = conn.transaction()?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE subjects SET sort_order = ?1 WHERE id = ?2",
            params![i as i64, id],
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

    #[test]
    fn create_and_list() {
        let conn = setup();
        let a = create(&conn, "Maths", "#E06C75").unwrap();
        let b = create(&conn, "English", "#61afef").unwrap();

        assert_eq!(a.name, "Maths");
        // Colour is normalised to lowercase.
        assert_eq!(a.color, "#e06c75");
        assert!(!a.archived);
        // New subjects append to the end.
        assert_eq!(a.sort_order, 0);
        assert_eq!(b.sort_order, 1);

        let all = list(&conn, false).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, a.id);
    }

    #[test]
    fn name_is_trimmed_and_required() {
        let conn = setup();
        let s = create(&conn, "  Physics  ", "#98c379").unwrap();
        assert_eq!(s.name, "Physics");

        assert!(matches!(
            create(&conn, "   ", "#98c379"),
            Err(SubjectError::EmptyName)
        ));
        let long = "x".repeat(MAX_NAME_CHARS + 1);
        assert!(matches!(
            create(&conn, &long, "#98c379"),
            Err(SubjectError::NameTooLong(_))
        ));
    }

    #[test]
    fn duplicate_names_are_rejected_with_a_clear_error() {
        let conn = setup();
        create(&conn, "History", "#98c379").unwrap();
        match create(&conn, "History", "#61afef") {
            Err(SubjectError::DuplicateName(n)) => assert_eq!(n, "History"),
            other => panic!("expected DuplicateName, got {other:?}"),
        }
    }

    #[test]
    fn invalid_colours_are_rejected() {
        let conn = setup();
        for bad in ["red", "#fff", "#12345g", "e06c75", ""] {
            assert!(
                matches!(create(&conn, "S", bad), Err(SubjectError::InvalidColor(_))),
                "'{bad}' should not be accepted"
            );
        }
    }

    #[test]
    fn update_patches_one_field_at_a_time() {
        let conn = setup();
        let s = create(&conn, "Chem", "#98c379").unwrap();

        let renamed = update(&conn, s.id, Some("Chemistry"), None).unwrap();
        assert_eq!(renamed.name, "Chemistry");
        assert_eq!(renamed.color, "#98c379", "colour must be left alone");

        let recoloured = update(&conn, s.id, None, Some("#61AFEF")).unwrap();
        assert_eq!(recoloured.name, "Chemistry");
        assert_eq!(recoloured.color, "#61afef");
    }

    #[test]
    fn archived_subjects_are_hidden_from_the_default_list() {
        let conn = setup();
        let s = create(&conn, "Latin", "#98c379").unwrap();
        set_archived(&conn, s.id, true).unwrap();

        assert!(list(&conn, false).unwrap().is_empty());
        assert_eq!(list(&conn, true).unwrap().len(), 1);
    }

    #[test]
    fn missing_ids_report_not_found() {
        let conn = setup();
        assert!(matches!(get(&conn, 999), Err(SubjectError::NotFound(999))));
        assert!(matches!(delete(&conn, 999), Err(SubjectError::NotFound(999))));
        assert!(matches!(
            set_archived(&conn, 999, true),
            Err(SubjectError::NotFound(999))
        ));
        assert!(matches!(
            update(&conn, 999, Some("x"), None),
            Err(SubjectError::NotFound(999))
        ));
    }

    #[test]
    fn deleting_a_subject_keeps_its_sessions() {
        let conn = setup();
        let s = create(&conn, "Biology", "#98c379").unwrap();
        crate::db::queries::insert_session(&conn, "work", 1500, Some(s.id), None).unwrap();

        delete(&conn, s.id).unwrap();

        let (count, subject): (i64, Option<i64>) = conn
            .query_row("SELECT COUNT(*), MAX(subject_id) FROM sessions", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(subject, None);
    }

    #[test]
    fn reorder_persists_the_given_order() {
        let mut conn = setup();
        let a = create(&conn, "A", "#98c379").unwrap();
        let b = create(&conn, "B", "#61afef").unwrap();
        let c = create(&conn, "C", "#c678dd").unwrap();

        reorder(&mut conn, &[c.id, a.id, b.id]).unwrap();

        let names: Vec<String> = list(&conn, false).unwrap().into_iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["C", "A", "B"]);
    }
}
