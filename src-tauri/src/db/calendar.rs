//! Calendar events use a half-open UTC interval supplied by the frontend from
//! local midnight boundaries. This preserves local weeks across DST changes.
use super::queries::SubjectFilter;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct WeekEvent {
    pub id: i64,
    pub started_at: i64,
    pub duration_secs: i64,
    pub subject_id: Option<i64>,
    pub subject_name: Option<String>,
    pub subject_color: Option<String>,
    pub task_id: Option<i64>,
    pub task_title: Option<String>,
    /// False for a round that was skipped or reset part way; `duration_secs`
    /// is then the time actually focused.
    pub completed: bool,
}

// ---------------------------------------------------------------------------
// Editing by hand (fork addition): the week calendar can add, move, resize and
// delete focus records. New ones are ordinary completed work sessions, so
// stats, charts, task totals and the Google Calendar sync all count them.
// Unfinished rounds can be moved, resized and deleted too, and stay unfinished.
// ---------------------------------------------------------------------------

/// A focus record as the calendar edits it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusRecord {
    pub started_at: i64,
    pub duration_secs: i64,
    pub subject_id: Option<i64>,
    pub task_id: Option<i64>,
}

/// Longest record that can be drawn by hand (a real round is far shorter).
pub const MAX_RECORD_SECS: i64 = 12 * 3600;

fn check(conn: &Connection, record: &FocusRecord, now: i64) -> Result<(), String> {
    if !(60..=MAX_RECORD_SECS).contains(&record.duration_secs) {
        return Err("时长要在 1 分钟到 12 小时之间 / A record lasts 1 minute to 12 hours".into());
    }
    if record.started_at < 0 {
        return Err("开始时间无效 / Invalid start time".into());
    }
    // A minute of slack for a record that ends "now".
    if record.started_at + record.duration_secs > now + 60 {
        return Err("记录不能结束在未来 / A record can't end in the future".into());
    }
    let exists = |table: &str, id: i64| -> Result<bool, String> {
        conn.query_row(&format!("SELECT 1 FROM {table} WHERE id = ?1"), [id], |_| Ok(()))
            .optional()
            .map(|row| row.is_some())
            .map_err(|e| e.to_string())
    };
    if let Some(id) = record.subject_id {
        if !exists("subjects", id)? {
            return Err("科目不存在 / That subject no longer exists".into());
        }
    }
    if let Some(id) = record.task_id {
        if !exists("tasks", id)? {
            return Err("任务不存在 / That task no longer exists".into());
        }
    }
    Ok(())
}

/// A focus record the calendar shows, by id; the round in progress, breaks and
/// rounds with no focus counted are not editable here.
fn existing(conn: &Connection, id: i64) -> Result<FocusRecord, String> {
    conn.query_row(
        "SELECT started_at, focused_secs, subject_id, task_id FROM sessions
         WHERE id = ?1 AND round_type = 'work' AND focused_secs > 0",
        [id],
        |r| Ok(FocusRecord { started_at: r.get(0)?, duration_secs: r.get(1)?, subject_id: r.get(2)?, task_id: r.get(3)? }),
    )
    .optional()
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "这条记录已经不存在 / That record no longer exists".into())
}

/// Add a completed focus record; returns its id.
pub fn create_record(conn: &Connection, record: &FocusRecord, now: i64) -> Result<i64, String> {
    check(conn, record, now)?;
    conn.execute(
        "INSERT INTO sessions (started_at, ended_at, round_type, duration_secs, focused_secs, completed, subject_id, task_id)
         VALUES (?1, ?2, 'work', ?3, ?3, 1, ?4, ?5)",
        params![record.started_at, record.started_at + record.duration_secs, record.duration_secs, record.subject_id, record.task_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// Change a record's time, subject or task; returns what it was before.
pub fn update_record(conn: &Connection, id: i64, record: &FocusRecord, now: i64) -> Result<FocusRecord, String> {
    let before = existing(conn, id)?;
    check(conn, record, now)?;
    conn.execute(
        "UPDATE sessions SET started_at = ?2, ended_at = ?3, duration_secs = ?4, focused_secs = ?4, subject_id = ?5, task_id = ?6 WHERE id = ?1",
        params![id, record.started_at, record.started_at + record.duration_secs, record.duration_secs, record.subject_id, record.task_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(before)
}

/// Delete a record; returns it, so it can be put back.
pub fn delete_record(conn: &Connection, id: i64) -> Result<FocusRecord, String> {
    let before = existing(conn, id)?;
    conn.execute("DELETE FROM sessions WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
    Ok(before)
}

pub fn get_week_events(
    conn: &Connection,
    start: i64,
    end: i64,
    subject: SubjectFilter,
) -> Result<Vec<WeekEvent>, String> {
    get_events(conn, start, end, subject, 8)
}

/// Same rows as the week calendar over a longer range, for the daily-total
/// and per-subject charts (which split sessions at local midnight themselves).
pub fn get_range_events(
    conn: &Connection,
    start: i64,
    end: i64,
    subject: SubjectFilter,
) -> Result<Vec<WeekEvent>, String> {
    get_events(conn, start, end, subject, 400)
}

fn get_events(
    conn: &Connection,
    start: i64,
    end: i64,
    subject: SubjectFilter,
    max_days: i64,
) -> Result<Vec<WeekEvent>, String> {
    let span = end.checked_sub(start).ok_or("Invalid calendar interval")?;
    if !(1..=max_days * 86400).contains(&span) || start < 0 || end > 253402300799 {
        return Err(format!("Interval must be between one second and {max_days} days"));
    }
    let (mode, subject_id) = match subject {
        SubjectFilter::All => (0, None),
        SubjectFilter::Uncategorized => (1, None),
        SubjectFilter::Subject(id) => (2, Some(id)),
    };
    let mut stmt = conn.prepare(
        "SELECT s.id, s.started_at, s.focused_secs, s.subject_id,
                subjects.name, subjects.color, s.task_id, tasks.title, s.completed
         FROM sessions s
         LEFT JOIN subjects ON subjects.id = s.subject_id
         LEFT JOIN tasks ON tasks.id = s.task_id
         WHERE s.round_type = 'work' AND s.focused_secs > 0
           AND s.started_at < ?2 AND s.started_at + s.focused_secs > ?1
           AND (?3 = 0 OR (?3 = 1 AND s.subject_id IS NULL) OR (?3 = 2 AND s.subject_id = ?4))
         ORDER BY s.started_at, s.id",
    ).map_err(|e| e.to_string())?;
    let events = stmt.query_map(params![start, end, mode, subject_id], |row| {
        Ok(WeekEvent {
            id: row.get(0)?, started_at: row.get(1)?, duration_secs: row.get(2)?,
            subject_id: row.get(3)?, subject_name: row.get(4)?,
            subject_color: row.get(5)?, task_id: row.get(6)?, task_title: row.get(7)?,
            completed: row.get(8)?,
        })
    }).map_err(|e| e.to_string())?;
    events.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO subjects (id,name,color,created_at) VALUES (1,'Math','#72251f',0);
             INSERT INTO tasks (id,title,subject_id,created_at) VALUES (1,'Chapter 7',1,0);
             INSERT INTO sessions (started_at,round_type,duration_secs,completed,subject_id,task_id) VALUES
                (900,'work',200,1,1,1),
                (1000,'work',300,1,NULL,NULL),
                (1100,'short-break',300,1,NULL,NULL),
                (1200,'work',300,0,NULL,NULL),
                (2000,'work',300,1,NULL,NULL),
                (800,'work',200,1,NULL,NULL);
             UPDATE sessions SET focused_secs = duration_secs WHERE round_type = 'work' AND completed = 1;"
        ).unwrap();
        conn
    }

    #[test]
    fn includes_cross_boundary_focus_and_excludes_breaks_incomplete_and_end() {
        let events = get_week_events(&setup(), 1000, 2000, SubjectFilter::All).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].task_title.as_deref(), Some("Chapter 7"));
        assert_eq!(events[0].subject_name.as_deref(), Some("Math"));
    }

    #[test]
    fn all_subject_filter_modes_work() {
        let conn = setup();
        assert_eq!(get_week_events(&conn,1000,2000,SubjectFilter::Uncategorized).unwrap().len(),1);
        assert_eq!(get_week_events(&conn,1000,2000,SubjectFilter::Subject(1)).unwrap().len(),1);
        assert!(get_week_events(&conn,1000,2000,SubjectFilter::Subject(99)).unwrap().is_empty());
    }

    #[test]
    fn deleted_subjects_and_tasks_do_not_erase_history() {
        let conn = setup();
        conn.execute("DELETE FROM tasks", []).unwrap();
        conn.execute("DELETE FROM subjects", []).unwrap();
        let events = get_week_events(&conn,1000,2000,SubjectFilter::Uncategorized).unwrap();
        assert_eq!(events.len(),2);
        assert!(events[0].task_title.is_none());
        assert!(events[0].subject_color.is_none());
    }

    #[test]
    fn validates_interval_and_allows_dst_weeks() {
        let conn = setup();
        for end in [1000 + 7*86400 - 3600, 1000 + 7*86400 + 3600] {
            assert!(get_week_events(&conn,1000,end,SubjectFilter::All).is_ok());
        }
        for (start,end) in [(1000,1000),(2000,1000),(0,9*86400),(i64::MIN,i64::MAX)] {
            assert!(get_week_events(&conn,start,end,SubjectFilter::All).is_err());
        }
    }

    const NOW: i64 = 100_000;

    fn record(started_at: i64, minutes: i64, subject_id: Option<i64>, task_id: Option<i64>) -> FocusRecord {
        FocusRecord { started_at, duration_secs: minutes * 60, subject_id, task_id }
    }

    #[test]
    fn records_added_by_hand_show_up_like_rounds() {
        let conn = setup();
        let id = create_record(&conn, &record(50_000, 40, Some(1), Some(1)), NOW).unwrap();
        let events = get_week_events(&conn, 49_000, 60_000, SubjectFilter::All).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!((events[0].id, events[0].duration_secs, events[0].task_id), (id, 2400, Some(1)));
        let ended: i64 = conn.query_row("SELECT ended_at FROM sessions WHERE id = ?1", [id], |r| r.get(0)).unwrap();
        assert_eq!(ended, 50_000 + 2400);
    }

    #[test]
    fn records_move_resize_and_change_subject() {
        let conn = setup();
        let id = create_record(&conn, &record(50_000, 25, None, None), NOW).unwrap();
        let before = update_record(&conn, id, &record(51_000, 50, Some(1), None), NOW).unwrap();
        assert_eq!(before, record(50_000, 25, None, None));
        let events = get_week_events(&conn, 49_000, 60_000, SubjectFilter::Subject(1)).unwrap();
        assert_eq!((events[0].started_at, events[0].duration_secs), (51_000, 3000));
    }

    #[test]
    fn deleted_records_can_be_put_back() {
        let conn = setup();
        let id = create_record(&conn, &record(50_000, 25, Some(1), Some(1)), NOW).unwrap();
        let removed = delete_record(&conn, id).unwrap();
        assert!(get_week_events(&conn, 49_000, 60_000, SubjectFilter::All).unwrap().is_empty());
        create_record(&conn, &removed, NOW).unwrap();
        assert_eq!(get_week_events(&conn, 49_000, 60_000, SubjectFilter::All).unwrap().len(), 1);
    }

    #[test]
    fn bad_records_are_refused() {
        let conn = setup();
        for bad in [
            record(50_000, 0, None, None),
            FocusRecord { started_at: 50_000, duration_secs: 59, subject_id: None, task_id: None },
            record(50_000, 12 * 60 + 1, None, None),
            record(-10, 25, None, None),
            record(NOW - 60, 25, None, None), // ends in the future
            record(50_000, 25, Some(99), None),
            record(50_000, 25, None, Some(99)),
        ] {
            assert!(create_record(&conn, &bad, NOW).is_err(), "{bad:?}");
        }
        // Ending right now is fine.
        assert!(create_record(&conn, &record(NOW - 1500, 25, None, None), NOW).is_ok());
    }

    #[test]
    fn unfinished_rounds_show_their_focused_time_and_can_be_edited() {
        let conn = setup();
        let id = crate::db::queries::insert_session(&conn, "work", 1500, None, None).unwrap();
        conn.execute("UPDATE sessions SET started_at = 50000 WHERE id = ?1", [id]).unwrap();
        crate::db::queries::complete_session(&conn, id, false, 600).unwrap();
        let events = get_week_events(&conn, 49_000, 60_000, SubjectFilter::All).unwrap();
        assert_eq!((events[0].id, events[0].duration_secs, events[0].completed), (id, 600, false));

        update_record(&conn, id, &record(50_000, 15, Some(1), None), NOW).unwrap();
        let events = get_week_events(&conn, 49_000, 60_000, SubjectFilter::All).unwrap();
        assert_eq!((events[0].duration_secs, events[0].completed), (900, false), "stays unfinished");
        assert_eq!(delete_record(&conn, id).unwrap(), record(50_000, 15, Some(1), None));
    }

    #[test]
    fn only_completed_focus_records_are_editable() {
        let conn = setup();
        conn.execute("INSERT INTO sessions (started_at,round_type,duration_secs,completed) VALUES (3000,'work',1500,0)", []).unwrap();
        let running = conn.last_insert_rowid();
        let brk: i64 = conn.query_row("SELECT id FROM sessions WHERE round_type = 'short-break'", [], |r| r.get(0)).unwrap();
        for id in [running, brk, 12345] {
            assert!(update_record(&conn, id, &record(4000, 25, None, None), NOW).is_err());
            assert!(delete_record(&conn, id).is_err());
        }
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM sessions WHERE id IN (?1, ?2)", [running, brk], |r| r.get(0)).unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn range_events_allow_a_year_but_not_more() {
        let conn = setup();
        assert_eq!(get_range_events(&conn, 0, 366 * 86400, SubjectFilter::All).unwrap().len(), 4);
        assert!(get_range_events(&conn, 0, 401 * 86400, SubjectFilter::All).is_err());
    }
}
