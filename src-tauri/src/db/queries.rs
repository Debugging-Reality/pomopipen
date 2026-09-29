use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Subject filtering
// ---------------------------------------------------------------------------

/// Which subjects a stats query covers.
///
/// Three states are needed, so `Option<i64>` is not enough: "everything",
/// "only this subject", and "only the rounds that had no subject at all".
///
/// Serialised for the frontend as `"all"`, `"uncategorized"`, or
/// `{ "subject": 3 }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubjectFilter {
    /// Every session, whatever its subject.
    #[default]
    All,
    /// Only sessions recorded while no subject was active.
    Uncategorized,
    /// Only sessions belonging to this subject id.
    Subject(i64),
}

impl SubjectFilter {
    /// SQL fragment appended to an existing WHERE clause.
    ///
    /// The id is an `i64` that came out of SQLite itself, never user-supplied
    /// text, so inlining it is injection-safe - and it keeps the positional
    /// parameters of the surrounding queries untouched.
    fn sql(&self) -> String {
        match self {
            SubjectFilter::All => String::new(),
            SubjectFilter::Uncategorized => " AND subject_id IS NULL".to_string(),
            SubjectFilter::Subject(id) => format!(" AND subject_id = {id}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Session CRUD (DATA-03)
// ---------------------------------------------------------------------------

/// Inserts a new session row when a round begins.
/// Returns the row ID so it can be passed to `complete_session` later.
pub fn insert_session(
    conn: &Connection,
    round_type: &str,
    duration_secs: u32,
    subject_id: Option<i64>,
    task_id: Option<i64>,
) -> Result<i64> {
    let started_at = unix_now();
    conn.execute(
        "INSERT INTO sessions (started_at, round_type, duration_secs, completed, subject_id, task_id)
         VALUES (?1, ?2, ?3, 0, ?4, ?5)",
        params![started_at, round_type, duration_secs, subject_id, task_id],
    )?;
    let id = conn.last_insert_rowid();
    log::debug!(
        "[db] session started: id={id} type={round_type} duration={duration_secs}s subject={subject_id:?} task={task_id:?}"
    );
    Ok(id)
}

/// Re-tag the in-flight work session when the user switches subject mid-round.
///
/// The session row is written on the first tick, so a subject picked later
/// would otherwise be ignored for the round already under way. Only rows with
/// no `ended_at` are touched, which is exactly the round in progress.
pub fn retag_open_work_session(
    conn: &Connection,
    subject_id: Option<i64>,
    task_id: Option<i64>,
) -> Result<usize> {
    conn.execute(
        "UPDATE sessions SET subject_id = ?1, task_id = ?2
         WHERE ended_at IS NULL AND round_type = 'work'",
        params![subject_id, task_id],
    )
}

/// Shortest unfinished stretch of focus that still counts toward study time
/// (the same minimum as a record drawn in the week calendar).
pub const MIN_FOCUS_SECS: u32 = 60;

/// Updates a session when the round ends: completed, skipped, or reset part
/// way. A completed round counts its full length; an unfinished one counts the
/// `elapsed_secs` actually focused, once that reaches `MIN_FOCUS_SECS`.
pub fn complete_session(
    conn: &Connection,
    session_id: i64,
    completed: bool,
    elapsed_secs: u32,
) -> Result<()> {
    let partial = (elapsed_secs >= MIN_FOCUS_SECS).then_some(elapsed_secs);
    conn.execute(
        "UPDATE sessions
            SET ended_at = ?1, completed = ?2, progress_secs = NULL,
                focused_secs = CASE WHEN ?2 = 1 THEN duration_secs ELSE ?3 END
          WHERE id = ?4",
        params![unix_now(), completed as i64, partial, session_id],
    )?;
    log::debug!("[db] session ended: id={session_id} completed={completed} elapsed={elapsed_secs}s");
    Ok(())
}

/// Checkpoint of the round in progress, so quitting mid-round loses at most
/// the last minute (see `close_abandoned_sessions`).
pub fn save_session_progress(conn: &Connection, session_id: i64, elapsed_secs: u32) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET progress_secs = ?1 WHERE id = ?2 AND ended_at IS NULL",
        params![elapsed_secs, session_id],
    )?;
    Ok(())
}

/// Close the rounds a previous run left open (the app quit, crashed, or the
/// machine shut down mid-round), counting the focus checkpointed for them.
/// Call once at startup, before the timer can open a new round.
pub fn close_abandoned_sessions(conn: &Connection) -> Result<usize> {
    let n = conn.execute(
        "UPDATE sessions
            SET ended_at = started_at + COALESCE(progress_secs, 0),
                focused_secs = CASE WHEN round_type = 'work' AND progress_secs >= ?1
                                    THEN progress_secs END,
                progress_secs = NULL
          WHERE ended_at IS NULL",
        params![MIN_FOCUS_SECS],
    )?;
    if n > 0 {
        log::info!("[db] closed {n} session(s) left open by the last run");
    }
    Ok(n)
}

// ---------------------------------------------------------------------------
// Stats queries
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct SessionStats {
    pub total_work_sessions: i64,
    pub completed_work_sessions: i64,
    /// Seconds focused in all work sessions, finished or not (`focused_secs`).
    pub total_work_secs: i64,
}

pub fn get_all_time_stats(conn: &Connection, filter: SubjectFilter) -> Result<SessionStats> {
    let f = filter.sql();

    let total_work_sessions: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM sessions WHERE round_type = 'work'{f}"),
        [],
        |r| r.get(0),
    )?;

    let completed_work_sessions: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM sessions WHERE round_type = 'work' AND completed = 1{f}"),
        [],
        |r| r.get(0),
    )?;

    let total_work_secs: i64 = conn.query_row(
        &format!(
            "SELECT COALESCE(SUM(focused_secs), 0)
             FROM sessions WHERE round_type = 'work'{f}"
        ),
        [],
        |r| r.get(0),
    )?;

    Ok(SessionStats {
        total_work_sessions,
        completed_work_sessions,
        total_work_secs,
    })
}

// ---------------------------------------------------------------------------
// Detailed stats queries (DATA-04)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct DailyStats {
    pub rounds: u32,
    pub focus_mins: u32,
    /// None when no work sessions were started today (avoids 0/0).
    pub completion_rate: Option<f32>,
    /// Completed work rounds per hour of the day (index 0 = midnight).
    pub by_hour: Vec<u32>,
}

#[derive(Debug, Serialize)]
pub struct DayStat {
    /// Local calendar date in "YYYY-MM-DD" format.
    pub date: String,
    pub rounds: u32,
}

#[derive(Debug, Serialize)]
pub struct HeatmapEntry {
    /// Local calendar date in "YYYY-MM-DD" format.
    pub date: String,
    pub count: u32,
}

#[derive(Debug, Serialize)]
pub struct StreakInfo {
    pub current: u32,
    pub longest: u32,
}

/// Completed work rounds and focus time for today (local calendar date).
pub fn get_daily_stats(conn: &Connection, filter: SubjectFilter) -> Result<DailyStats> {
    let f = filter.sql();

    let today: String = conn.query_row(
        "SELECT date('now', 'localtime')",
        [],
        |r| r.get(0),
    )?;

    let total: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM sessions
             WHERE round_type = 'work'
             AND date(started_at, 'unixepoch', 'localtime') = ?1{f}"
        ),
        [&today],
        |r| r.get(0),
    )?;

    let completed: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM sessions
             WHERE round_type = 'work' AND completed = 1
             AND date(started_at, 'unixepoch', 'localtime') = ?1{f}"
        ),
        [&today],
        |r| r.get(0),
    )?;

    let focus_secs: i64 = conn.query_row(
        &format!(
            "SELECT COALESCE(SUM(focused_secs), 0) FROM sessions
             WHERE round_type = 'work'
             AND date(started_at, 'unixepoch', 'localtime') = ?1{f}"
        ),
        [&today],
        |r| r.get(0),
    )?;

    let mut by_hour = vec![0u32; 24];
    let mut stmt = conn.prepare(&format!(
        "SELECT CAST(strftime('%H', datetime(started_at, 'unixepoch', 'localtime')) AS INTEGER) as h,
                COUNT(*) as cnt
         FROM sessions
         WHERE round_type = 'work' AND completed = 1
         AND date(started_at, 'unixepoch', 'localtime') = ?1{f}
         GROUP BY h"
    ))?;
    let rows = stmt.query_map([&today], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, u32>(1)?)))?;
    for row in rows.flatten() {
        let (h, cnt) = row;
        if (0..24).contains(&h) {
            by_hour[h as usize] = cnt;
        }
    }

    Ok(DailyStats {
        rounds: completed as u32,
        focus_mins: ((focus_secs + 30) / 60) as u32,
        completion_rate: if total > 0 { Some(completed as f32 / total as f32) } else { None },
        by_hour,
    })
}

/// Completed work rounds per local calendar day for the last 7 days.
pub fn get_weekly_stats(conn: &Connection, filter: SubjectFilter) -> Result<Vec<DayStat>> {
    let f = filter.sql();
    let mut stmt = conn.prepare(&format!(
        "SELECT date(started_at, 'unixepoch', 'localtime') as day,
                COUNT(*) as rounds
         FROM sessions
         WHERE round_type = 'work' AND completed = 1
         AND date(started_at, 'unixepoch', 'localtime') >= date('now', 'localtime', '-6 days'){f}
         GROUP BY day
         ORDER BY day"
    ))?;
    let rows = stmt.query_map([], |r| Ok(DayStat { date: r.get(0)?, rounds: r.get(1)? }))?
        .collect();
    rows
}

/// Completed work rounds per local calendar day, all time (no date limit).
/// The frontend slices this into per-year views for navigation.
pub fn get_heatmap_data(conn: &Connection, filter: SubjectFilter) -> Result<Vec<HeatmapEntry>> {
    let f = filter.sql();
    let mut stmt = conn.prepare(&format!(
        "SELECT date(started_at, 'unixepoch', 'localtime') as day,
                COUNT(*) as cnt
         FROM sessions
         WHERE round_type = 'work' AND completed = 1{f}
         GROUP BY day
         ORDER BY day"
    ))?;
    let rows = stmt.query_map([], |r| Ok(HeatmapEntry { date: r.get(0)?, count: r.get(1)? }))?
        .collect();
    rows
}

/// Current and longest study streaks (consecutive local calendar days).
/// A day counts once it has any focus time, unfinished rounds included.
/// A streak stays active until midnight: if yesterday had sessions but today does not,
/// the streak is still counted as current.
pub fn get_streak(conn: &Connection, filter: SubjectFilter) -> Result<StreakInfo> {
    let f = filter.sql();

    let today: String = conn.query_row(
        "SELECT date('now', 'localtime')",
        [],
        |r| r.get(0),
    )?;

    let mut stmt = conn.prepare(&format!(
        "SELECT date(started_at, 'unixepoch', 'localtime') as day
         FROM sessions
         WHERE round_type = 'work' AND focused_secs > 0{f}
         GROUP BY day
         ORDER BY day"
    ))?;
    let days: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .flatten()
        .collect();

    Ok(compute_streak(&days, &today))
}

// ---------------------------------------------------------------------------
// Subject breakdown
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct SubjectTotal {
    /// `None` for the uncategorised bucket.
    pub subject_id: Option<i64>,
    /// Empty for the uncategorised bucket - the frontend supplies the label.
    pub name: String,
    /// Empty for the uncategorised bucket.
    pub color: String,
    /// Completed rounds only.
    pub rounds: u32,
    /// Includes focus from rounds that didn't finish.
    pub focus_secs: i64,
}

/// Focus time per subject over the last `days` local calendar days
/// (`None` = all time), unfinished rounds included.
///
/// Uses a LEFT JOIN so sessions whose subject was deleted, and sessions that
/// never had one, both land in a single `subject_id: None` bucket. That keeps
/// the breakdown summing to the same total as the unfiltered stats.
pub fn get_subject_breakdown(conn: &Connection, days: Option<u32>) -> Result<Vec<SubjectTotal>> {
    let range = match days {
        Some(d) => format!(
            " AND date(s.started_at, 'unixepoch', 'localtime') >= date('now', 'localtime', '-{} days')",
            d.saturating_sub(1)
        ),
        None => String::new(),
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT s.subject_id,
                COALESCE(sub.name, ''),
                COALESCE(sub.color, ''),
                SUM(s.completed),
                COALESCE(SUM(s.focused_secs), 0)
         FROM sessions s
         LEFT JOIN subjects sub ON sub.id = s.subject_id
         WHERE s.round_type = 'work' AND s.focused_secs > 0{range}
         GROUP BY s.subject_id
         ORDER BY SUM(s.focused_secs) DESC"
    ))?;
    // Bound to a local first: returning the collect() straight from the tail
    // expression keeps `stmt` borrowed past its own drop.
    let rows = stmt
        .query_map([], |r| {
            Ok(SubjectTotal {
                subject_id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
                rounds: r.get(3)?,
                focus_secs: r.get(4)?,
            })
        })?
        .collect();
    rows
}

// ---------------------------------------------------------------------------
// Streak helpers
// ---------------------------------------------------------------------------

/// Convert a "YYYY-MM-DD" string to a day number for arithmetic comparison.
/// Uses the proleptic Gregorian calendar; absolute value is arbitrary — only
/// differences between dates matter.
fn date_to_day_num(s: &str) -> Option<i32> {
    let mut parts = s.splitn(3, '-');
    let y: i32 = parts.next()?.parse().ok()?;
    let m: i32 = parts.next()?.parse().ok()?;
    let d: i32 = parts.next()?.parse().ok()?;
    let y = if m <= 2 { y - 1 } else { y };
    let m = if m <= 2 { m + 12 } else { m };
    Some(y * 365 + y / 4 - y / 100 + y / 400 + (153 * m - 457) / 5 + d)
}

pub fn compute_streak(days: &[String], today: &str) -> StreakInfo {
    let nums: Vec<i32> = days.iter().filter_map(|s| date_to_day_num(s)).collect();
    if nums.is_empty() {
        return StreakInfo { current: 0, longest: 0 };
    }

    let today_n = match date_to_day_num(today) {
        Some(n) => n,
        None => return StreakInfo { current: 0, longest: 0 },
    };

    // Current streak — alive if most recent session day is today or yesterday.
    let last = *nums.last().unwrap();
    let current = if last == today_n || last == today_n - 1 {
        let mut count = 0u32;
        let mut expected = last;
        for &n in nums.iter().rev() {
            if n == expected {
                count += 1;
                expected -= 1;
            } else {
                break;
            }
        }
        count
    } else {
        0
    };

    // Longest streak.
    let mut longest = 1u32;
    let mut run = 1u32;
    for i in 1..nums.len() {
        if nums[i] == nums[i - 1] + 1 {
            run += 1;
            if run > longest { longest = run; }
        } else {
            run = 1;
        }
    }

    StreakInfo { current, longest }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run(&conn).unwrap();
        conn
    }

    #[test]
    fn insert_and_complete_session() {
        let conn = setup();
        let id = insert_session(&conn, "work", 1500, None, None).unwrap();
        assert!(id > 0);

        complete_session(&conn, id, true, 1500).unwrap();

        let completed: i64 = conn
            .query_row(
                "SELECT completed FROM sessions WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(completed, 1);
    }

    #[test]
    fn stats_empty_db() {
        let conn = setup();
        let stats = get_all_time_stats(&conn, SubjectFilter::All).unwrap();
        assert_eq!(stats.total_work_sessions, 0);
        assert_eq!(stats.completed_work_sessions, 0);
        assert_eq!(stats.total_work_secs, 0);
    }

    #[test]
    fn compute_streak_empty() {
        let info = compute_streak(&[], "2024-03-15");
        assert_eq!(info.current, 0);
        assert_eq!(info.longest, 0);
    }

    #[test]
    fn compute_streak_active_today() {
        let days = vec!["2024-03-13".to_string(), "2024-03-14".to_string(), "2024-03-15".to_string()];
        let info = compute_streak(&days, "2024-03-15");
        assert_eq!(info.current, 3);
        assert_eq!(info.longest, 3);
    }

    #[test]
    fn compute_streak_active_until_midnight() {
        // Yesterday had sessions, today does not — streak still live.
        let days = vec!["2024-03-13".to_string(), "2024-03-14".to_string()];
        let info = compute_streak(&days, "2024-03-15");
        assert_eq!(info.current, 2);
    }

    #[test]
    fn compute_streak_broken() {
        // Last session was 2 days ago — streak is broken.
        let days = vec!["2024-03-12".to_string(), "2024-03-13".to_string()];
        let info = compute_streak(&days, "2024-03-15");
        assert_eq!(info.current, 0);
    }

    #[test]
    fn compute_streak_longest_across_break() {
        let days = vec![
            "2024-03-01".to_string(), "2024-03-02".to_string(), "2024-03-03".to_string(),
            "2024-03-10".to_string(), "2024-03-11".to_string(),
        ];
        let info = compute_streak(&days, "2024-03-11");
        assert_eq!(info.current, 2);
        assert_eq!(info.longest, 3);
    }

    #[test]
    fn get_daily_stats_empty() {
        let conn = setup();
        let stats = get_daily_stats(&conn, SubjectFilter::All).unwrap();
        assert_eq!(stats.rounds, 0);
        assert_eq!(stats.focus_mins, 0);
        assert!(stats.completion_rate.is_none());
        assert_eq!(stats.by_hour.len(), 24);
    }

    #[test]
    fn get_weekly_stats_empty() {
        let conn = setup();
        let stats = get_weekly_stats(&conn, SubjectFilter::All).unwrap();
        assert!(stats.is_empty());
    }

    #[test]
    fn get_heatmap_data_empty() {
        let conn = setup();
        let entries = get_heatmap_data(&conn, SubjectFilter::All).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn focus_mins_rounds_to_nearest_minute() {
        let conn = setup();

        // 339 s = 5:39 → rounds up to 6 min (remainder 39 ≥ 30).
        let id1 = insert_session(&conn, "work", 339, None, None).unwrap();
        complete_session(&conn, id1, true, 0).unwrap();
        let stats = get_daily_stats(&conn, SubjectFilter::All).unwrap();
        assert_eq!(stats.focus_mins, 6, "339 s should round to 6 min");

        // Reset and test round-down: 324 s = 5:24 → rounds down to 5 min (remainder 24 < 30).
        let conn2 = setup();
        let id2 = insert_session(&conn2, "work", 324, None, None).unwrap();
        complete_session(&conn2, id2, true, 0).unwrap();
        let stats2 = get_daily_stats(&conn2, SubjectFilter::All).unwrap();
        assert_eq!(stats2.focus_mins, 5, "324 s should round to 5 min");

        // Exact minute boundary: 1500 s = 25:00 → stays 25 min.
        let conn3 = setup();
        let id3 = insert_session(&conn3, "work", 1500, None, None).unwrap();
        complete_session(&conn3, id3, true, 0).unwrap();
        let stats3 = get_daily_stats(&conn3, SubjectFilter::All).unwrap();
        assert_eq!(stats3.focus_mins, 25, "1500 s should be exactly 25 min");
    }

    #[test]
    fn stats_counts_correctly() {
        let conn = setup();

        let id1 = insert_session(&conn, "work", 1500, None, None).unwrap();
        complete_session(&conn, id1, true, 1500).unwrap();

        let id2 = insert_session(&conn, "work", 1500, None, None).unwrap();
        complete_session(&conn, id2, false, 30).unwrap(); // skipped too soon to count

        let _id3 = insert_session(&conn, "short-break", 300, None, None).unwrap();

        let stats = get_all_time_stats(&conn, SubjectFilter::All).unwrap();
        assert_eq!(stats.total_work_sessions, 2);
        assert_eq!(stats.completed_work_sessions, 1);
        assert_eq!(stats.total_work_secs, 1500);
    }

    #[test]
    fn unfinished_rounds_count_their_focus_but_not_as_rounds() {
        let conn = setup();
        let done = insert_session(&conn, "work", 1500, None, None).unwrap();
        complete_session(&conn, done, true, 1500).unwrap();
        // Had to leave 10 minutes into the next round.
        let left = insert_session(&conn, "work", 1500, None, None).unwrap();
        complete_session(&conn, left, false, 600).unwrap();

        let all = get_all_time_stats(&conn, SubjectFilter::All).unwrap();
        assert_eq!((all.completed_work_sessions, all.total_work_secs), (1, 2100));
        let today = get_daily_stats(&conn, SubjectFilter::All).unwrap();
        assert_eq!((today.rounds, today.focus_mins), (1, 35));
        let subjects = get_subject_breakdown(&conn, None).unwrap();
        assert_eq!((subjects[0].rounds, subjects[0].focus_secs), (1, 2100));
    }

    #[test]
    fn a_round_left_open_counts_its_last_checkpoint() {
        let conn = setup();
        let quit = insert_session(&conn, "work", 1500, None, None).unwrap();
        save_session_progress(&conn, quit, 60).unwrap();
        save_session_progress(&conn, quit, 720).unwrap();
        let barely = insert_session(&conn, "work", 1500, None, None).unwrap();
        save_session_progress(&conn, barely, 30).unwrap();
        let _break = insert_session(&conn, "short-break", 300, None, None).unwrap();

        assert_eq!(close_abandoned_sessions(&conn).unwrap(), 3);
        assert_eq!(close_abandoned_sessions(&conn).unwrap(), 0, "only once");
        assert_eq!(get_all_time_stats(&conn, SubjectFilter::All).unwrap().total_work_secs, 720);
        let (started, ended): (i64, i64) = conn
            .query_row("SELECT started_at, ended_at FROM sessions WHERE id = ?1", [quit], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(ended - started, 720);
    }

    #[test]
    fn progress_is_not_saved_after_the_round_ends() {
        let conn = setup();
        let id = insert_session(&conn, "work", 1500, None, None).unwrap();
        complete_session(&conn, id, false, 300).unwrap();
        save_session_progress(&conn, id, 900).unwrap();
        close_abandoned_sessions(&conn).unwrap();
        assert_eq!(get_all_time_stats(&conn, SubjectFilter::All).unwrap().total_work_secs, 300);
    }

    #[test]
    fn a_day_with_only_unfinished_focus_keeps_the_streak() {
        let conn = setup();
        let too_short = insert_session(&conn, "work", 1500, None, None).unwrap();
        complete_session(&conn, too_short, false, 30).unwrap();
        assert_eq!(get_streak(&conn, SubjectFilter::All).unwrap().current, 0);

        let left = insert_session(&conn, "work", 1500, None, None).unwrap();
        complete_session(&conn, left, false, 600).unwrap();
        assert_eq!(get_streak(&conn, SubjectFilter::All).unwrap().current, 1);
    }
}
