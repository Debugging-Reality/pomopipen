import type { FocusRecord, WeekEvent } from '../types';

/** Days since the start of `date`'s week (0–6); weeks start on Sunday or Monday. */
export const dayOfWeek = (date: Date, mondayFirst = false) => (date.getDay() + 7 - (mondayFirst ? 1 : 0)) % 7;

/** Local midnight on the first day of `date`'s week. */
export function startOfWeek(date = new Date(), mondayFirst = false): Date {
  const d = new Date(date);
  d.setHours(0, 0, 0, 0);
  d.setDate(d.getDate() - dayOfWeek(d, mondayFirst));
  return d;
}

export function addDays(date: Date, count: number): Date {
  const next = new Date(date);
  next.setDate(next.getDate() + count);
  return next;
}

export function dateKey(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

/** Longest break between two rounds of one subject that still counts as one block. */
export const MERGE_GAP_SECS = 10 * 60;

/** Consecutive focus rounds of one subject on one day, drawn as one block (the
 *  same rule decides the events synced to Google Calendar, see src-tauri/src/gcal). */
export interface FocusBlock extends WeekEvent {
  /** Seconds actually focused; the block itself also spans the breaks between rounds. */
  focus_secs: number;
  /** Completed rounds; `members` also holds the unfinished ones. */
  rounds: number;
  /** Distinct task titles, in order. `task_title` is set only when there is exactly one. */
  tasks: string[];
  /** The rounds in the block, in order — what editing changes. */
  members: WeekEvent[];
}

const finished = (e: WeekEvent) => (e.completed === false ? 0 : 1);

/** Merge each round into the block before it when it is the same subject on the
 *  same local day and starts at most MERGE_GAP_SECS after that block ends. */
export function mergeRounds(events: WeekEvent[]): FocusBlock[] {
  const blocks: FocusBlock[] = [];
  const sorted = events.filter(e => Number.isFinite(e.started_at) && Number.isFinite(e.duration_secs) && e.duration_secs > 0)
    .sort((a, b) => a.started_at - b.started_at || a.id - b.id);
  for (const event of sorted) {
    const last = blocks[blocks.length - 1];
    const lastEnd = last ? last.started_at + last.duration_secs : 0;
    if (last && last.subject_id === event.subject_id && event.started_at - lastEnd <= MERGE_GAP_SECS
      && dateKey(new Date(last.started_at * 1000)) === dateKey(new Date(event.started_at * 1000))) {
      last.duration_secs = Math.max(lastEnd, event.started_at + event.duration_secs) - last.started_at;
      last.focus_secs += event.duration_secs;
      last.rounds += finished(event);
      if (event.task_title && !last.tasks.includes(event.task_title)) last.tasks.push(event.task_title);
      last.task_title = last.tasks.length === 1 ? last.tasks[0] : null;
      last.members.push(event);
    } else {
      blocks.push({ ...event, focus_secs: event.duration_secs, rounds: finished(event), tasks: event.task_title ? [event.task_title] : [], members: [event] });
    }
  }
  return blocks;
}

// ── Editing by hand (drag in the week calendar, or the record editor) ──

/** Minutes that drawn and dragged records snap to. */
export const SNAP_MINUTES = 5;
/** Longest record the backend accepts (db::calendar::MAX_RECORD_SECS). */
export const MAX_RECORD_SECS = 12 * 3600;

export const snapMinutes = (minutes: number, step = SNAP_MINUTES) => Math.round(minutes / step) * step;

/** Unix seconds `minute` minutes after local midnight of `day`, on the wall clock. */
export function atMinute(day: Date, minute: number): number {
  const d = new Date(day);
  d.setHours(0, 0, 0, 0);
  d.setMinutes(minute);
  return Math.floor(d.getTime() / 1000);
}

/** Move a start time by whole days and minutes on the wall clock (DST-safe). */
export function shiftStart(startedAt: number, days: number, minutes: number): number {
  const d = new Date(startedAt * 1000);
  d.setDate(d.getDate() + days);
  d.setMinutes(d.getMinutes() + minutes);
  return Math.floor(d.getTime() / 1000);
}

export const toRecord = (e: WeekEvent): FocusRecord =>
  ({ started_at: e.started_at, duration_secs: e.duration_secs, subject_id: e.subject_id, task_id: e.task_id });

/** The records after dragging a block: every round moves by the same wall-clock
 *  amount, keeping the gaps between them. */
export function movedRecords(block: FocusBlock, days: number, minutes: number): [WeekEvent, FocusRecord][] {
  const delta = shiftStart(block.started_at, days, minutes) - block.started_at;
  return block.members.map(m => [m, { ...toRecord(m), started_at: m.started_at + delta }]);
}

/** Dragging a block's lower edge changes its last round, never below one snap step. */
export function resizedRecord(block: FocusBlock, minutes: number): [WeekEvent, FocusRecord] {
  const last = block.members[block.members.length - 1];
  const duration = Math.min(MAX_RECORD_SECS, Math.max(SNAP_MINUTES * 60, last.duration_secs + minutes * 60));
  return [last, { ...toRecord(last), duration_secs: duration }];
}

export interface CalendarSegment<T extends WeekEvent = WeekEvent> {
  key: string;
  event: T;
  day: number;
  startMinute: number;
  endMinute: number;
  seconds: number;
  column: number;
  columns: number;
}

/** Split at local midnights, clipping to the selected week. */
export function splitEvents<T extends WeekEvent>(events: T[], weekStart: Date): CalendarSegment<T>[] {
  const result: CalendarSegment<T>[] = [];
  for (const event of events) {
    if (!Number.isFinite(event.started_at) || !Number.isFinite(event.duration_secs) || event.duration_secs <= 0) continue;
    const eventStart = event.started_at * 1000;
    const eventEnd = eventStart + event.duration_secs * 1000;
    for (let day = 0; day < 7; day++) {
      const midnight = addDays(weekStart, day).getTime();
      const nextMidnight = addDays(weekStart, day + 1).getTime();
      const from = Math.max(midnight, eventStart);
      const to = Math.min(nextMidnight, eventEnd);
      if (to <= from) continue;
      const startDate = new Date(from);
      const endDate = new Date(to);
      const startMinute = startDate.getHours() * 60 + startDate.getMinutes() + startDate.getSeconds() / 60;
      let endMinute = to === nextMidnight ? 1440 : endDate.getHours() * 60 + endDate.getMinutes() + endDate.getSeconds() / 60;
      // During a fall-back hour, wall-clock end can precede start. Keep the block visible.
      if (endMinute <= startMinute) endMinute = Math.min(1440, startMinute + (to - from) / 60000);
      result.push({ key: `${event.id}-${day}`, event, day, startMinute, endMinute,
        seconds: (to - from) / 1000, column: 0, columns: 1 });
    }
  }
  return result;
}

/** Assign lanes to connected groups of overlapping blocks, including minimum hit height. */
export function layoutSegments<T extends WeekEvent>(segments: CalendarSegment<T>[]): CalendarSegment<T>[] {
  const placed = segments.map(s => ({ ...s }));
  for (let day = 0; day < 7; day++) {
    const events = placed.filter(s => s.day === day).sort((a, b) => a.startMinute - b.startMinute || a.event.id - b.event.id);
    let group: CalendarSegment<T>[] = [];
    let laneEnds: number[] = [];
    const flush = () => { for (const s of group) s.columns = laneEnds.length; group = []; laneEnds = []; };
    for (const segment of events) {
      if (group.length && segment.startMinute >= Math.max(...laneEnds)) flush();
      let lane = laneEnds.findIndex(end => end <= segment.startMinute);
      if (lane === -1) lane = laneEnds.length;
      segment.column = lane;
      laneEnds[lane] = Math.max(segment.endMinute, segment.startMinute + 20);
      group.push(segment);
    }
    flush();
  }
  return placed;
}

/** Free minutes directly above a block in its day — how far a decoration may
 *  rise above it (Infinity when nothing ends earlier that day). */
export function minutesFreeAbove(segments: CalendarSegment[], segment: CalendarSegment): number {
  let nearest = -Infinity;
  for (const s of segments) {
    if (s !== segment && s.day === segment.day && s.endMinute <= segment.startMinute + 1e-9) nearest = Math.max(nearest, s.endMinute);
  }
  return nearest === -Infinity ? Infinity : segment.startMinute - nearest;
}

export interface DayTotal {
  key: string; // YYYY-MM-DD, local
  date: Date; // local midnight
  seconds: number;
  /** Seconds per subject id; `null` = uncategorized. */
  bySubject: Map<number | null, number>;
}

/** Focus seconds per local day for `days` days from `start` (a local midnight),
 *  splitting sessions that cross midnight like the week calendar does. */
export function dailyTotals(events: WeekEvent[], start: Date, days: number): DayTotal[] {
  const bounds = Array.from({ length: days + 1 }, (_, i) => addDays(start, i).getTime());
  const result: DayTotal[] = bounds.slice(0, days).map(ms => {
    const date = new Date(ms);
    return { key: dateKey(date), date, seconds: 0, bySubject: new Map() };
  });
  for (const event of events) {
    if (!Number.isFinite(event.started_at) || !Number.isFinite(event.duration_secs) || event.duration_secs <= 0) continue;
    const from = event.started_at * 1000;
    const to = from + event.duration_secs * 1000;
    // First day whose end is after the session start (days are ~86.4M ms; DST shifts by ≤1 h).
    let day = Math.max(0, Math.floor((from - bounds[0]) / 86_400_000) - 1);
    while (day < days && bounds[day + 1] <= from) day++;
    for (; day < days && bounds[day] < to; day++) {
      const seconds = (Math.min(to, bounds[day + 1]) - Math.max(from, bounds[day])) / 1000;
      if (seconds <= 0) continue;
      const total = result[day];
      total.seconds += seconds;
      const subject = event.subject_id ?? null;
      total.bySubject.set(subject, (total.bySubject.get(subject) ?? 0) + seconds);
    }
  }
  return result;
}

/** Trailing mean over `window` values (fewer at the very start of the series). */
export function movingAverage(values: number[], window: number): number[] {
  const size = Math.max(1, Math.floor(window));
  let sum = 0;
  return values.map((value, i) => {
    sum += value;
    if (i >= size) sum -= values[i - size];
    return sum / Math.min(i + 1, size);
  });
}

export function visibleHours(segments: CalendarSegment[]): [number, number] {
  if (!segments.length) return [8, 22];
  return [Math.max(0, Math.min(8, Math.floor(Math.min(...segments.map(s => s.startMinute)) / 60))),
    Math.min(24, Math.max(22, Math.ceil(Math.max(...segments.map(s => s.endMinute)) / 60)))];
}
