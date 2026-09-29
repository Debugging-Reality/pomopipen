// Isolated visual fixture. Never writes to the app's database.
import { get } from 'svelte/store';
import { settings } from '../../src/lib/stores/settings';
import { collection } from '../../src/lib/themes/collection';
import { startOfWeek, addDays, dateKey } from '../../src/lib/utils/calendar';
import type { TimerState, Settings, Subject, Task, WeekEvent, FocusRecord, SubjectFilter, HeatmapStats, SubjectTotal, Theme, GcalStatus, Jot, JotCapture } from '../../src/lib/types';
export let snapshot: TimerState = { round_type: 'work', previous_round_type: '', elapsed_secs: 0,
  total_secs: 1500, is_running: false, is_paused: false, work_round_number: 1,
  work_rounds_total: 4, session_work_count: 0 };
const listeners = new Set<(state: TimerState) => void>();
export const getTimerState = async () => snapshot;
export const onRoundChange = async (fn: (state: TimerState) => void) => { listeners.add(fn); return () => { listeners.delete(fn); }; };
export const onTimerReset = async (_fn: (state: TimerState) => void) => () => {};
const subjectListeners = new Set<() => void>();
export const onSubjectsChanged = async (fn: () => void) => { subjectListeners.add(fn); return () => { subjectListeners.delete(fn); }; };
// ?lang=en gives English sample names (product images).
const params = new URLSearchParams(typeof location !== 'undefined' ? location.search : '');
const EN = params.get('lang') === 'en';
// id, English, Chinese, tomato variety, older free-pick color. English (product
// images) uses the varieties; Chinese keeps some non-variety colors so the
// one-click conversion in Settings → Subjects has something to convert.
const SUBJECTS: [number, string, string, string, string][] = [
  [1, 'Calculus', '高等数学', '#E9A60C', '#E9A60C'],
  [2, 'English', '大学英语', '#2B7F3E', '#3B82F6'],
  [3, 'Linear Algebra', '线性代数', '#9C2F55', '#8B5CF6'],
  [4, 'Algorithms', '专业课 408', '#D33526', '#EF4444'],
  [5, 'History', '政治', '#F07A22', '#F07A22'],
  [6, 'Reading', '课外阅读', '#F27C8E', '#EC4899'],
];
let subjectRows: Subject[] = SUBJECTS.map(([id, en, zh, variety, older], i) => ({
  id, name: EN ? en : zh, color: EN ? variety : older, archived: id === 6, created_at: 0, sort_order: i }));
export const subjectsList = async (includeArchived = false): Promise<Subject[]> =>
  subjectRows.filter(s => includeArchived || !s.archived).map(s => ({ ...s }));
export const subjectsUpdate = async (id: number, name?: string, color?: string) => {
  subjectRows = subjectRows.map(s => s.id === id ? { ...s, name: name ?? s.name, color: color ?? s.color } : s);
  subjectListeners.forEach(fn => fn());
};
export const subjectsCreate = async (_name: string, _color: string) => {};
export const subjectsSetArchived = async (_id: number, _archived: boolean) => {};
export const subjectsDelete = async (_id: number) => {};
export const subjectsReorder = async (_ids: number[]) => {};
export const statsGetDetailed = async () => ({ today: { rounds: 3, focus_mins: 75, completion_rate: 1, by_hour: [] }, week: [], streak: { current: 5, longest: 9 } });
export const subjectsSetActive = async (_id: number | null) => get(settings);
export const timerToggle = async () => {};
export const timerRestartRound = async () => {};
export const timerSkip = async () => {};
export const timerReset = async () => {};
export const audioPlayClick = async () => {};
export const importBackground = async (_sourcePath: string, _target: 'timer' | 'calendar') => '';
export function advance() {
  const round_type = snapshot.round_type === 'work' ? 'short-break' : snapshot.round_type === 'short-break' ? 'long-break' : 'work';
  snapshot = { ...snapshot, round_type, previous_round_type: snapshot.round_type, elapsed_secs: 0,
    total_secs: round_type === 'work' ? 1500 : round_type === 'short-break' ? 300 : 900 };
  listeners.forEach(fn => fn(snapshot));
  return snapshot;
}
export const setSetting = async (key: string, value: string): Promise<Settings> => {
  const parsed = ['stats_zoom', 'basket_frame_opacity', 'basket_floor_opacity', 'time_work_secs', 'time_short_break_secs', 'time_long_break_secs'].includes(key) ? Number(value)
    : value === 'true' || value === 'false' ? value === 'true' : value;
  const result = { ...get(settings), [key]: parsed };
  settings.set(result);
  return result;
};

// The real stats and tasks windows (?view=stats|tasks) load their own settings and themes.
export const getThemes = async (): Promise<Theme[]> =>
  Promise.all(collection.map(async t => ({ ...await (await fetch(`/themes/${t.id}.json`)).json(), is_custom: false })));
export const getSettings = async (): Promise<Settings> => {
  const theme = collection.find(c => c.id === params.get('theme')) ?? collection[0];
  return { ...get(settings), theme_mode: 'light', theme_light: theme.name, language: EN ? 'en' : 'zh',
    active_subject_id: 1, active_task_id: 1, ...(params.get('bg') === 'none' ? { theme_backgrounds: '{}' } : {}) };
};
export const onSettingsChanged = async (_fn: (settings: Settings) => void) => () => {};

// Google Calendar sync (?view=sync): ?gcal=none|ready|connected|error picks the state.
const GCAL = params.get('gcal') ?? 'connected';
let gcal: GcalStatus = { client_id: GCAL === 'none' ? null : '123456789012-a1b2c3d4e5f6.apps.googleusercontent.com',
  client_embedded: false, connected: GCAL === 'connected' || GCAL === 'error', account: 'you@example.com',
  auto_sync: true, syncing: false, connecting: false, last_sync: Math.floor(Date.now() / 1000) - 180,
  last_error: GCAL === 'error' ? '连不上 Google：经代理 http://127.0.0.1:7897 也没连上，请确认代理节点现在能打开 Google / Cannot reach Google through the proxy http://127.0.0.1:7897; check that it can open Google right now' : null,
  last_result: { blocks: 38, created: 3, updated: 1, deleted: 0 },
  proxy: params.get('proxy') === 'none' ? null : 'http://127.0.0.1:7897' };
export const gcalStatus = async () => gcal;
export const gcalImportClient = async (_path: string) => gcal;
export const gcalConnect = async () => (gcal = { ...gcal, connected: true });
export const gcalCancelConnect = async () => {};
export const gcalSyncNow = async () => gcal;
export const gcalDisconnect = async () => (gcal = { ...gcal, connected: false });
export const gcalSetAuto = async (enabled: boolean) => (gcal = { ...gcal, auto_sync: enabled });
export const onGcalStatus = async (_fn: (s: GcalStatus) => void) => () => {};
export const openJsonFilePicker = async () => null;
export const onThemesChanged = async (_fn: (themes: Theme[]) => void) => () => {};

// ── Tasks: [subject (0 = none), English, Chinese, estimate min, logged min, done] ──
const TASKS: [number, string, string, number | null, number, boolean][] = [
  [1, 'Ch. 3 review problems', '第三章 综合题', 90, 75, false],
  [1, 'Limits worksheet', '极限例题', 50, 55, false],
  [1, 'Integration by parts drills', '分部积分练习', 45, 0, false],
  [1, 'Ch. 2 summary notes', '第二章 总结笔记', 30, 35, true],
  [2, 'Unit 5 reading', 'Unit 5 阅读', 30, 25, false],
  [2, 'Listening · Section B', '听力 Section B', 25, 0, false],
  [2, 'Essay draft: technology', '作文：科技', 60, 30, false],
  [2, 'Vocabulary list 12', '单词 List 12', 20, 20, true],
  [3, 'Eigenvalues problem set', '特征值习题', 75, 50, false],
  [3, 'Matrix rank proofs', '矩阵的秩', null, 0, false],
  [4, 'Page replacement exercises', '页面置换习题', 50, 50, false],
  [4, 'Process scheduling notes', '进程调度笔记', 90, 100, false],
  [5, 'Modern history outline', '近代史纲要', 40, 0, false],
  [0, 'Plan next week', '安排下周计划', 15, 0, false],
];
let taskRows: Task[] = TASKS.map(([subject, en, zh, est, logged, done], i) => ({ id: i + 1, title: EN ? en : zh,
  subject_id: subject || null, done, est_minutes: est, created_at: 0, completed_at: done ? 0 : null, sort_order: i, actual_secs: logged * 60 }));
const taskListeners = new Set<() => void>();
const changeTasks = (fn: (t: Task) => Task) => { taskRows = taskRows.map(fn); taskListeners.forEach(l => l()); };
export const onTasksChanged = async (fn: () => void) => { taskListeners.add(fn); return () => { taskListeners.delete(fn); }; };
export const tasksList = async (includeDone = false): Promise<Task[]> => taskRows.filter(t => includeDone || !t.done).map(t => ({ ...t }));
export const tasksCreate = async (title: string, subjectId: number | null, est: number | null) => {
  taskRows.push({ id: taskRows.length + 1, title, subject_id: subjectId, done: false, est_minutes: est, created_at: 0, completed_at: null, sort_order: taskRows.length, actual_secs: 0 });
  taskListeners.forEach(l => l());
};
export const tasksRename = async (id: number, title: string) => changeTasks(t => t.id === id ? { ...t, title } : t);
export const tasksSetEstimate = async (id: number, est: number | null) => changeTasks(t => t.id === id ? { ...t, est_minutes: est } : t);
export const tasksSetDone = async (id: number, done: boolean) => changeTasks(t => t.id === id ? { ...t, done } : t);
export const tasksDelete = async (id: number) => { taskRows = taskRows.filter(t => t.id !== id); taskListeners.forEach(l => l()); };
export const tasksSetActive = async (id: number | null) => {
  const result = { ...get(settings), active_task_id: id };
  settings.set(result);
  return result;
};

// ── Study history for the charts, heatmap and calendar ──
export const onSessionsCleared = async (_fn: () => void) => () => {};
/** Event colors. Empty: each subject's tomato variety. App.svelte fills it with another theme's palette. */
export const subjectPalette: string[] = [];
const eventColor = (id: number) => subjectPalette.length ? subjectPalette[(id - 1) % subjectPalette.length] : SUBJECTS[id - 1][3];
// This week is placed by hand so the calendar reads like a real one:
// [day (0 = Sunday), start, minutes, subject (0 = none), English task, Chinese task, unfinished]
const WEEK: [number, string, number, number, string?, string?, boolean?][] = [
  [0, '08:30', 25, 1, 'Limits', '极限例题'], [0, '09:00', 25, 1, 'Limits', '极限例题'], [0, '09:30', 25, 1, 'Limits', '极限例题'],
  [0, '10:30', 50, 2, 'Unit 5 reading', 'Unit 5 阅读'], [0, '13:30', 25, 5], [0, '14:00', 25, 5],
  [0, '15:00', 90, 4, 'Process scheduling', '进程调度'], [0, '17:00', 25, 3], [0, '19:30', 50, 6, 'Sapiens', '《人类简史》'],
  [1, '08:00', 50, 2, 'Listening · Sec. B', '听力 Section B'], [1, '09:00', 25, 1], [1, '09:30', 25, 1], [1, '10:00', 14, 1, undefined, undefined, true],
  [1, '14:00', 90, 4, 'Paging', '页式存储'], [1, '16:00', 25, 6], [1, '16:30', 25, 6],
  [1, '19:00', 50, 1, 'Integration drills', '分部积分练习'], [1, '20:30', 25, 3], [1, '21:00', 25, 3],
  [2, '09:00', 90, 1, 'Ch. 3 review', '第三章 综合题'], [2, '11:00', 25, 3], [2, '14:00', 50, 2, 'Unit 6 reading', 'Unit 6 阅读'],
  [2, '15:00', 12, 0, undefined, undefined, true], [2, '16:00', 50, 4, 'Page replacement', '页面置换'], [2, '19:30', 90, 5, 'Modern history', '近代史纲要'],
  [3, '08:30', 25, 5], [3, '09:00', 25, 5], [3, '10:00', 50, 3, 'Eigenvalues', '特征值'],
  [3, '13:00', 25, 1], [3, '13:30', 25, 1], [3, '14:00', 25, 1], [3, '15:00', 50, 2, 'Essay draft', '作文'],
  [3, '16:30', 25, 6], [3, '20:00', 50, 4, 'Scheduling quiz', '调度小测'],
  [4, '09:00', 50, 4, 'Deadlocks', '死锁'], [4, '10:00', 50, 4, "Banker's algorithm", '银行家算法'],
  [4, '14:00', 25, 2], [4, '14:30', 25, 2], [4, '15:00', 25, 2], [4, '16:00', 25, 1], [4, '16:30', 25, 1],
  [4, '19:00', 90, 3, 'Problem set 4', '线代习题 4'], [4, '21:00', 25, 2, 'Vocabulary', '单词'],
  [5, '10:00', 90, 1, 'Weekly quiz', '周测 · 真题'], [5, '15:00', 50, 6, 'Sapiens', '《人类简史》'],
  [5, '19:30', 50, 2, 'Mock listening', '听力模考'], [5, '20:30', 25, 5],
  [6, '09:30', 25, 3], [6, '10:00', 25, 3], [6, '11:00', 50, 4, 'File systems', '文件系统'],
  [6, '16:00', 50, 5, 'Marxism · Ch. 2', '马原第二章'], [6, '20:00', 50, 1, 'Review mistakes', '错题回顾'],
];
function history(): WeekEvent[] {
  const events: WeekEvent[] = [];
  const push = (start: Date, minutes: number, subject: number, en?: string, zh?: string, unfinished = false) => events.push({
    id: events.length + 1, started_at: start.getTime() / 1000, duration_secs: minutes * 60,
    subject_id: subject || null, subject_name: subject ? subjectRows.find(s => s.id === subject)?.name ?? null : null,
    subject_color: subject ? eventColor(subject) : null, task_title: (EN ? en : zh) ?? null,
    task_id: taskRows.find(t => t.title === (EN ? en : zh))?.id ?? null, completed: !unfinished });
  let seed = 7;
  const rand = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
  const week = startOfWeek();
  const today = new Date(); today.setHours(0, 0, 0, 0);
  // Earlier weeks since mid-January: deterministic, with rest days and a mix of 25/50/90-minute sessions.
  for (let back = 255; back >= 0; back--) {
    const day = addDays(today, -back);
    if (day >= week) break;
    if (rand() < 0.18) continue; // rest day
    const rounds = 2 + Math.floor(rand() * (back < 60 ? 7 : 5));
    let minute = 8 * 60 + Math.floor(rand() * 90);
    for (let r = 0; r < rounds && minute < 21 * 60; r++) {
      const pick = Math.floor(rand() * rand() * 6.9); // 0–6, weighted to the first subjects; 6 = none
      const subject = pick === 6 ? 0 : pick + 1;
      const minutes = [25, 25, 25, 25, 50, 90][Math.floor(rand() * 6)];
      const start = new Date(day); start.setHours(0, minute, 0, 0);
      push(start, minutes, subject, subject === 1 ? 'Ch. 7 · Limits' : undefined, subject === 1 ? '第七章 · 极限' : undefined);
      minute += minutes + 5 + Math.floor(rand() * 40);
    }
  }
  for (const [day, time, minutes, subject, en, zh, unfinished] of WEEK) {
    const [h, m] = time.split(':').map(Number);
    const start = addDays(week, day); start.setHours(h, m, 0, 0);
    push(start, minutes, subject, en, zh, unfinished);
  }
  return events;
}
// Records edited in the calendar (?view=stats&tab=calendar): the sample history
// is copied on the first edit and changed in memory, with the backend's rules.
let edited: WeekEvent[] | null = null;
const allEvents = () => edited ?? history();
const sessionListeners = new Set<() => void>();
export const onSessionsChanged = async (fn: () => void) => { sessionListeners.add(fn); return () => { sessionListeners.delete(fn); }; };
function asEvent(id: number, r: FocusRecord): WeekEvent {
  const subject = subjectRows.find(s => s.id === r.subject_id);
  if (r.duration_secs < 60 || r.duration_secs > 12 * 3600) throw '时长要在 1 分钟到 12 小时之间 / A record lasts 1 minute to 12 hours';
  if (r.started_at + r.duration_secs > Date.now() / 1000 + 60) throw "记录不能结束在未来 / A record can't end in the future";
  return { id, started_at: r.started_at, duration_secs: r.duration_secs, subject_id: r.subject_id,
    subject_name: subject?.name ?? null, subject_color: subject ? eventColor(subject.id) : null,
    task_id: r.task_id, task_title: taskRows.find(t => t.id === r.task_id)?.title ?? null, completed: true };
}
const changed = () => setTimeout(() => sessionListeners.forEach(l => l()), 30);
export const sessionsCreate = async (r: FocusRecord) => {
  const events = allEvents();
  const id = Math.max(0, ...events.map(e => e.id)) + 1;
  edited = [...events, asEvent(id, r)];
  changed();
  return id;
};
export const sessionsUpdate = async (id: number, r: FocusRecord) => {
  const next = asEvent(id, r);
  edited = allEvents().map(e => e.id === id ? next : e);
  changed();
};
export const sessionsDelete = async (id: number): Promise<FocusRecord> => {
  const gone = allEvents().find(e => e.id === id);
  if (!gone) throw '这条记录已经不存在 / That record no longer exists';
  edited = allEvents().filter(e => e.id !== id);
  changed();
  return { started_at: gone.started_at, duration_secs: gone.duration_secs, subject_id: gone.subject_id, task_id: gone.task_id };
};
const overlapping = (start: number, end: number) =>
  allEvents().filter(e => e.started_at < end && e.started_at + e.duration_secs > start);
export const statsGetRangeEvents = async (start: number, end: number, _subject?: unknown) => overlapping(start, end);
export const statsGetWeekEvents = statsGetRangeEvents;
const inFilter = (e: WeekEvent, filter?: SubjectFilter) =>
  !filter || filter === 'all' ? true : filter === 'uncategorized' ? e.subject_id === null : e.subject_id === filter.subject;
export const statsGetHeatmap = async (filter?: SubjectFilter): Promise<HeatmapStats> => {
  const events = allEvents().filter(e => inFilter(e, filter));
  const counts = new Map<string, number>();
  for (const e of events) {
    const key = dateKey(new Date(e.started_at * 1000));
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  const entries = [...counts].sort(([a], [b]) => a.localeCompare(b)).map(([date, count]) => ({ date, count }));
  let longest = 0;
  let run = 0;
  entries.forEach(({ date }, i) => {
    const follows = i > 0 && dateKey(addDays(new Date(`${entries[i - 1].date}T00:00:00`), 1)) === date;
    run = follows ? run + 1 : 1;
    longest = Math.max(longest, run);
  });
  return { entries, total_rounds: events.length, longest_streak: longest,
    total_hours: Math.round(events.reduce((sum, e) => sum + e.duration_secs, 0) / 3600) };
};
export const statsGetSubjectBreakdown = async (_days?: number): Promise<SubjectTotal[]> => {
  const totals = new Map<number | null, SubjectTotal>();
  for (const e of allEvents()) {
    const total = totals.get(e.subject_id) ?? { subject_id: e.subject_id, name: e.subject_name ?? '', color: e.subject_color ?? '', rounds: 0, focus_secs: 0 };
    total.rounds += 1;
    total.focus_secs += e.duration_secs;
    totals.set(e.subject_id, total);
  }
  return [...totals.values()].sort((a, b) => b.focus_secs - a.focus_secs);
};

// ── Jots (碎碎念, ?view=jots): [minutes ago, English, Chinese, subject (0 = none), mid-focus, handled minutes ago, task] ──
const JOTS: [number, string, string, number, boolean, number?, boolean?][] = [
  [4, 'Call Mum back tonight', '晚上给妈妈回个电话', 1, true],
  [11, 'Problem 7: try Lagrange multipliers instead', '第 7 题换拉格朗日乘数法试试', 1, true],
  [18, 'Hotpot after this week’s exams!!', '好想吃火锅……考完这周就去', 1, true],
  [70, 'Ask roommate when the library opens tomorrow', '问问室友明天图书馆几点开门', 0, false],
  [26 * 60, 'Order: B5 grid notebooks ×3', '下单：B5 方格本 ×3', 2, false],
  [40, 'Print the Unit 5 word list', '把 Unit 5 单词表打印出来', 2, true, 9],
  [95, 'Sort out Ch. 3 mistakes', '整理第三章错题', 1, true, 30, true],
];
const nowSecs = () => Math.floor(Date.now() / 1000);
let jotRows: Jot[] = JOTS.map(([ago, en, zh, subject, focus, handled, task], i) => ({
  id: i + 1, body: EN ? en : zh, created_at: nowSecs() - ago * 60, subject_id: subject || null, in_focus: focus,
  done_at: handled === undefined ? null : nowSecs() - handled * 60, task_id: null }));
for (const jot of jotRows.filter((_, i) => JOTS[i][6])) {
  const id = taskRows.length + 1;
  taskRows.push({ id, title: jot.body, subject_id: jot.subject_id, done: false, est_minutes: null, created_at: 0, completed_at: null, sort_order: taskRows.length, actual_secs: 0 });
  jot.task_id = id;
}
const jotListeners = new Set<() => void>();
const jotsChanged = () => jotListeners.forEach(l => l());
const findJot = (id: number) => { const j = jotRows.find(r => r.id === id); if (!j) throw `no jot with id ${id}`; return j; };
/** The harness tells the mock whether a focus round is running, as the backend would know. */
export let jotInFocus = false;
export const setJotInFocus = (v: boolean) => { jotInFocus = v; };
export const onJotsChanged = async (fn: () => void) => { jotListeners.add(fn); return () => { jotListeners.delete(fn); }; };
export const onJotCapture = async (_fn: (c: JotCapture) => void) => () => {};
export const setWindowVisibility = async (_visible: boolean) => {};
export const jotsList = async (): Promise<Jot[]> => jotRows.map(j => ({ ...j }));
export const jotsCreate = async (body: string): Promise<Jot> => {
  const jot: Jot = { id: Math.max(0, ...jotRows.map(j => j.id)) + 1, body: body.trim(), created_at: nowSecs(), done_at: null,
    task_id: null, subject_id: get(settings).active_subject_id, in_focus: jotInFocus };
  jotRows.push(jot); jotsChanged();
  return jot;
};
export const jotsEdit = async (id: number, body: string) => { findJot(id).body = body.trim(); jotsChanged(); return findJot(id); };
export const jotsSetDone = async (id: number, done: boolean) => { const j = findJot(id); j.done_at = done ? (j.done_at ?? nowSecs()) : null; jotsChanged(); return { ...j }; };
export const jotsToTask = async (id: number, subjectId: number | null): Promise<Task> => {
  const j = findJot(id);
  const task: Task = { id: taskRows.length + 1, title: j.body.split(/\s+/).join(' '), subject_id: subjectId, done: false, est_minutes: null, created_at: 0, completed_at: null, sort_order: taskRows.length, actual_secs: 0 };
  taskRows.push(task); taskListeners.forEach(l => l());
  j.task_id = task.id; j.done_at = nowSecs(); jotsChanged();
  return task;
};
export const jotsUntask = async (id: number) => {
  const j = findJot(id);
  taskRows = taskRows.filter(t => t.id !== j.task_id); taskListeners.forEach(l => l());
  j.task_id = null; j.done_at = null; jotsChanged();
  return { ...j };
};
export const jotsDelete = async (id: number) => { const j = findJot(id); jotRows = jotRows.filter(r => r.id !== id); jotsChanged(); return { ...j }; };
export const jotsRestore = async (jot: Jot) => { jotRows.push({ ...jot }); jotsChanged(); return jot; };
export const jotsClearHandled = async () => { const n = jotRows.filter(j => j.done_at !== null).length; jotRows = jotRows.filter(j => j.done_at === null); jotsChanged(); return n; };
