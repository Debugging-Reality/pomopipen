//! Local focus rounds → merged blocks → Google Calendar event bodies.
//!
//! The merge rule is the one the week calendar uses (`mergeRounds` in
//! src/lib/utils/calendar.ts): a round joins the block before it when it is
//! the same subject on the same local day and starts at most ten minutes
//! after that block ends.
use chrono::{DateTime, Local, NaiveDate, SecondsFormat, TimeZone, Utc};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Longest break between two rounds of one subject that still counts as one block.
pub const MERGE_GAP_SECS: i64 = 10 * 60;
/// Every event PomoPipen writes carries this private extended property; sync
/// never updates or deletes an event without it.
pub const MARKER: (&str, &str) = ("src", "pomopipen");

#[derive(Debug, Clone)]
pub struct Round {
    pub started_at: i64,
    pub duration_secs: i64,
    pub subject_id: Option<i64>,
    pub subject_name: Option<String>,
    pub subject_color: Option<String>,
    pub task_title: Option<String>,
    /// False for a round skipped or reset part way (`duration_secs` is then
    /// the focus it had): its time counts, but not as a round.
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub start: i64,
    pub end: i64,
    /// Seconds focused; the block itself also spans the breaks between rounds.
    pub focus_secs: i64,
    /// Completed rounds only.
    pub rounds: u32,
    pub subject_id: Option<i64>,
    pub subject_name: Option<String>,
    pub subject_color: Option<String>,
    /// Distinct task titles, in order.
    pub tasks: Vec<String>,
}

/// Focus rounds, finished or not, that started at or after `since` (unix seconds).
pub fn load_rounds(conn: &Connection, since: i64) -> rusqlite::Result<Vec<Round>> {
    let mut stmt = conn.prepare(
        "SELECT s.started_at, s.focused_secs, s.subject_id, subjects.name, subjects.color, tasks.title, s.completed
         FROM sessions s
         LEFT JOIN subjects ON subjects.id = s.subject_id
         LEFT JOIN tasks ON tasks.id = s.task_id
         WHERE s.round_type = 'work' AND s.focused_secs > 0 AND s.started_at >= ?1
         ORDER BY s.started_at, s.id",
    )?;
    let rows = stmt.query_map(params![since], |r| {
        Ok(Round {
            started_at: r.get(0)?,
            duration_secs: r.get(1)?,
            subject_id: r.get(2)?,
            subject_name: r.get(3)?,
            subject_color: r.get(4)?,
            task_title: r.get(5)?,
            completed: r.get(6)?,
        })
    })?;
    rows.collect()
}

fn local_day(ts: i64) -> Option<NaiveDate> {
    Local.timestamp_opt(ts, 0).earliest().map(|d| d.date_naive())
}

/// Local midnight `days_back` days before today, as unix seconds.
pub fn local_midnight_days_ago(days_back: u64) -> i64 {
    let day = Local::now().date_naive() - chrono::Days::new(days_back);
    day.and_hms_opt(0, 0, 0)
        .and_then(|t| Local.from_local_datetime(&t).earliest())
        .map_or(0, |t| t.timestamp())
}

/// Merge rounds (in start order) into blocks.
pub fn merge(rounds: &[Round]) -> Vec<Block> {
    let mut sorted: Vec<&Round> = rounds.iter().filter(|r| r.duration_secs > 0).collect();
    sorted.sort_by_key(|r| r.started_at);
    let mut blocks: Vec<Block> = Vec::new();
    for r in sorted {
        let end = r.started_at + r.duration_secs;
        if let Some(last) = blocks.last_mut() {
            if last.subject_id == r.subject_id
                && r.started_at - last.end <= MERGE_GAP_SECS
                && local_day(last.start) == local_day(r.started_at)
            {
                last.end = last.end.max(end);
                last.focus_secs += r.duration_secs;
                last.rounds += r.completed as u32;
                if let Some(task) = &r.task_title {
                    if !last.tasks.contains(task) {
                        last.tasks.push(task.clone());
                    }
                }
                continue;
            }
        }
        blocks.push(Block {
            start: r.started_at,
            end,
            focus_secs: r.duration_secs,
            rounds: r.completed as u32,
            subject_id: r.subject_id,
            subject_name: r.subject_name.clone(),
            subject_color: r.subject_color.clone(),
            tasks: r.task_title.iter().cloned().collect(),
        });
    }
    blocks
}

/// Stable event id: the same block (subject + first round) always maps to the
/// same event, so re-running a sync updates instead of duplicating. Google
/// allows `[a-v0-9]{5,1024}`; hex digits qualify.
pub fn event_id(block: &Block) -> String {
    let subject = block.subject_id.map_or_else(|| "none".to_string(), |id| id.to_string());
    let digest = Sha256::digest(format!("{subject}|{}", block.start));
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("plg{}", &hex[..40])
}

pub fn rfc3339(ts: i64) -> String {
    DateTime::<Utc>::from_timestamp(ts, 0)
        .unwrap_or_default()
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn parse_time(value: &Value) -> Option<i64> {
    DateTime::parse_from_rfc3339(value.get("dateTime")?.as_str()?).ok().map(|t| t.timestamp())
}

/// "25m", "2h05m".
fn hours_minutes(secs: i64) -> String {
    let minutes = (secs + 30) / 60;
    if minutes < 60 {
        format!("{minutes}m")
    } else {
        format!("{}h{:02}m", minutes / 60, minutes % 60)
    }
}

/// Google Calendar's eleven event colors as its web UI draws them (colorId → hex).
const EVENT_COLORS: [(&str, &str); 11] = [
    ("1", "#7986CB"), ("2", "#33B679"), ("3", "#8E24AA"), ("4", "#E67C73"),
    ("5", "#F6BF26"), ("6", "#F4511E"), ("7", "#039BE5"), ("8", "#616161"),
    ("9", "#3F51B5"), ("10", "#0B8043"), ("11", "#D50000"),
];

fn oklab(hex: &str) -> Option<[f64; 3]> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let channel = |i: usize| -> Option<f64> {
        let v = f64::from(u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()?) / 255.0;
        Some(if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) })
    };
    let (r, g, b) = (channel(0)?, channel(2)?, channel(4)?);
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    Some([
        0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    ])
}

/// The Google event color closest to a subject color (OKLab, lightness at 0.4×
/// like the tomato-variety matching); gray for no subject.
pub fn color_id(hex: Option<&str>) -> &'static str {
    let Some(target) = hex.and_then(oklab) else { return "8" };
    EVENT_COLORS
        .iter()
        .filter_map(|(id, c)| oklab(c).map(|p| {
            (*id, (0.4 * (p[0] - target[0])).hypot(p[1] - target[1]).hypot(p[2] - target[2]))
        }))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or("8", |(id, _)| id)
}

/// The event PomoPipen wants in Google Calendar for this block.
pub fn event_body(block: &Block, zh: bool) -> Value {
    let subject = block
        .subject_name
        .clone()
        .unwrap_or_else(|| if zh { "未分类".into() } else { "Uncategorized".into() });
    let focus = hours_minutes(block.focus_secs);
    let mut lines = vec![match (zh, block.rounds) {
        (true, 0) => format!("专注 {focus}"),
        (true, n) => format!("{n} 个番茄 · 专注 {focus}"),
        (false, 0) => format!("{focus} focus"),
        (false, n) => format!("{n} {} · {focus} focus", if n == 1 { "round" } else { "rounds" }),
    }];
    if !block.tasks.is_empty() {
        lines.push(if zh {
            format!("任务：{}", block.tasks.join("、"))
        } else {
            format!("Tasks: {}", block.tasks.join(", "))
        });
    }
    lines.push(String::new());
    lines.push(if zh {
        "由 PomoPipen 自动同步；在这里做的修改会在下次同步时被覆盖。".into()
    } else {
        "Synced by PomoPipen; edits made here are replaced on the next sync.".into()
    });
    json!({
        "id": event_id(block),
        "summary": format!("🍅 {subject} · {focus}"),
        "description": lines.join("\n"),
        "start": { "dateTime": rfc3339(block.start) },
        "end": { "dateTime": rfc3339(block.end) },
        "colorId": color_id(block.subject_color.as_deref()),
        // A study log: shown as free time, never a reminder.
        "transparency": "transparent",
        "reminders": { "useDefault": false },
        "extendedProperties": { "private": { "src": "pomopipen" } },
    })
}

/// Whether an event from Google carries our marker.
pub fn is_ours(event: &Value) -> bool {
    event.pointer("/extendedProperties/private/src").and_then(Value::as_str) == Some(MARKER.1)
}

/// Start of an event from Google, unix seconds.
pub fn event_start(event: &Value) -> Option<i64> {
    event.get("start").and_then(parse_time)
}

/// Whether `remote` already shows what `wanted` describes (times compared as
/// instants, since Google rewrites them in the calendar's time zone).
pub fn matches(remote: &Value, wanted: &Value) -> bool {
    ["summary", "description", "colorId", "transparency"].iter().all(|k| remote.get(k) == wanted.get(k))
        && event_start(remote) == event_start(wanted)
        && remote.get("end").and_then(parse_time) == wanted.get("end").and_then(parse_time)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(h: u32, m: u32) -> i64 {
        Local.with_ymd_and_hms(2026, 9, 21, h, m, 0).earliest().unwrap().timestamp()
    }
    fn round(start: i64, minutes: i64, subject: Option<i64>, task: Option<&str>) -> Round {
        Round {
            started_at: start,
            duration_secs: minutes * 60,
            subject_id: subject,
            subject_name: subject.map(|id| format!("S{id}")),
            subject_color: Some("#E9A60C".into()),
            task_title: task.map(String::from),
            completed: true,
        }
    }

    #[test]
    fn merges_like_the_week_calendar() {
        let blocks = merge(&[
            round(at(9, 30), 25, Some(1), Some("Limits")),
            round(at(8, 30), 25, Some(1), Some("Limits")),
            round(at(9, 0), 25, Some(1), Some("Series")),
            round(at(10, 5), 25, Some(1), None),  // 10-minute gap: same block
            round(at(10, 41), 25, Some(1), None), // 11 minutes: new block
            round(at(11, 10), 25, Some(2), None), // another subject
            round(at(23, 40), 25, Some(1), None),
            round(at(23, 40) + 30 * 60, 25, Some(1), None), // next local day
        ]);
        let shape: Vec<(i64, u32)> = blocks.iter().map(|b| (b.start, b.rounds)).collect();
        assert_eq!(shape, vec![(at(8, 30), 4), (at(10, 41), 1), (at(11, 10), 1), (at(23, 40), 1), (at(23, 40) + 1800, 1)]);
        assert_eq!(blocks[0].end, at(10, 30));
        assert_eq!(blocks[0].focus_secs, 100 * 60);
        assert_eq!(blocks[0].tasks, vec!["Limits", "Series"]);
        // Uncategorized rounds merge with each other too.
        assert_eq!(merge(&[round(at(8, 0), 25, None, None), round(at(8, 30), 25, None, None)]).len(), 1);
    }

    #[test]
    fn event_ids_are_stable_and_valid() {
        let block = merge(&[round(at(8, 0), 25, Some(3), None)]).remove(0);
        let id = event_id(&block);
        assert_eq!(id, event_id(&block.clone()));
        assert_eq!(id.len(), 43);
        assert!(id.chars().all(|c| c.is_ascii_digit() || ('a'..='v').contains(&c)), "{id}");
        let later = merge(&[round(at(8, 0), 25, Some(3), None), round(at(8, 30), 25, Some(3), None)]).remove(0);
        assert_eq!(event_id(&later), id, "a block that grows keeps its event");
        assert_ne!(event_id(&merge(&[round(at(8, 0), 25, Some(4), None)])[0]), id);
    }

    #[test]
    fn subject_colors_map_to_the_nearest_google_color() {
        assert_eq!(color_id(Some("#D33526")), "11"); // red tomato → Tomato
        assert_eq!(color_id(Some("#E9A60C")), "5"); // golden → Banana
        assert_eq!(color_id(Some("#2B7F3E")), "10"); // green zebra → Basil
        assert_eq!(color_id(Some("#F07A22")), "6"); // orange → Tangerine
        assert_eq!(color_id(Some("#3B82F6")), "7"); // blue → Peacock
        assert_eq!(color_id(None), "8");
        assert_eq!(color_id(Some("not a color")), "8");
    }

    #[test]
    fn event_body_describes_the_block_and_round_trips() {
        let block = merge(&[
            round(at(14, 2), 25, Some(1), Some("Ch. 7")),
            round(at(14, 32), 25, Some(1), Some("Ch. 7")),
        ])
        .remove(0);
        let zh = event_body(&block, true);
        assert_eq!(zh["summary"], "🍅 S1 · 50m");
        assert!(zh["description"].as_str().unwrap().starts_with("2 个番茄 · 专注 50m\n任务：Ch. 7\n"));
        assert!(is_ours(&zh));
        assert_eq!(event_start(&zh), Some(at(14, 2)));
        // Google echoes times with the calendar's offset; still the same event.
        let mut remote = zh.clone();
        let shifted = DateTime::<Utc>::from_timestamp(block.start, 0).unwrap().with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap());
        remote["start"]["dateTime"] = json!(shifted.to_rfc3339());
        assert!(matches(&remote, &zh));
        remote["summary"] = json!("edited by hand");
        assert!(!matches(&remote, &zh));
        assert_eq!(event_body(&block, false)["description"].as_str().unwrap().lines().next(), Some("2 rounds · 50m focus"));
        assert_eq!(hours_minutes(125 * 60), "2h05m");
    }

    #[test]
    fn unfinished_rounds_add_focus_but_not_rounds() {
        let left = Round { completed: false, ..round(at(14, 32), 10, Some(1), None) };
        let block = merge(&[round(at(14, 2), 25, Some(1), None), left.clone()]).remove(0);
        assert_eq!((block.rounds, block.focus_secs, block.end), (1, 35 * 60, at(14, 42)));
        let alone = merge(&[left]).remove(0);
        assert_eq!(event_body(&alone, true)["description"].as_str().unwrap().lines().next(), Some("专注 10m"));
        assert_eq!(event_body(&alone, false)["description"].as_str().unwrap().lines().next(), Some("10m focus"));
    }
}
