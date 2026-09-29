import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { splitEvents, layoutSegments, startOfWeek, dayOfWeek, addDays, visibleHours, dateKey, dailyTotals, movingAverage, minutesFreeAbove, mergeRounds, movedRecords, resizedRecord, atMinute, shiftStart, snapMinutes } from '../src/lib/utils/calendar.ts';
import { subjectShares, donutArc } from '../src/lib/utils/charts.ts';
import { parseBackgrounds, backgroundFor, withBackground } from '../src/lib/themes/backgrounds.ts';
import { findPeriod } from '../src/lib/utils/patternTile.ts';
import { timerLayout, coverTileWidth } from '../src/lib/utils/timerLayout.ts';
import { didAdvanceRound, nextBackgroundIndex } from '../src/lib/utils/background.ts';
import { luminance, readableInk, readableAccent, themedInk } from '../src/lib/utils/color.ts';
import { collection, subjectPaletteFor } from '../src/lib/themes/collection.ts';
import { VARIETIES, varietyFor, nearestVarieties, subjectVarieties } from '../src/lib/themes/varieties.ts';
import { drumScale, drumAngle, parseDuration, durationKey } from '../src/lib/utils/drumScale.ts';
import { splitJots, foldsInFocus, jotWhen, badgeText, isSendKey } from '../src/lib/utils/jots.ts';

const event = (id, date, minutes, extra = {}) => ({ id, started_at: date.getTime()/1000, duration_secs: minutes*60,
  subject_id: null, subject_name: null, subject_color: null, task_id: null, task_title: null, completed: true, ...extra });

test('all approved themes provide distinct coordinated subject palettes', () => {
  assert.deepEqual(collection.map(t => t.id), ['classic-tomato', 'cherry-soda', 'citrus-club', 'berry-planet']);
  const ratio=(a,b)=>(Math.max(luminance(a),luminance(b))+.05)/(Math.min(luminance(a),luminance(b))+.05);
  for (const item of collection) {
    const theme=JSON.parse(fs.readFileSync(new URL(`../static/themes/${item.id}.json`,import.meta.url),'utf8'));
    assert.equal(theme.name,item.name);
    assert.deepEqual(subjectPaletteFor(theme),item.subjectColors);
    assert.ok(item.subjectColors.length>=8);
    for (const color of item.subjectColors) {
      assert.match(color,/^#[\da-f]{6}$/i);
      assert.ok(ratio(themedInk(color,theme.colors),color)>=4.5,`${item.id} ${color}`);
    }
    for(const bg of ['--color-background','--color-background-light'])
      assert.ok(ratio(theme.colors['--color-foreground'],theme.colors[bg])>=4.5,`${item.id} ${bg}`);
  }
});

test('local Sunday week across month/year boundaries', () => {
  assert.equal(dateKey(startOfWeek(new Date(2026,0,1,18))), '2025-12-28');
});

test('cross-midnight and cross-week focus are clipped without losing duration', () => {
  const start = new Date(2026,8,20);
  const pieces = splitEvents([event(1,new Date(2026,8,20,23,50),30)], start);
  assert.equal(pieces.length,2);
  assert.equal(pieces[0].endMinute,1440);
  assert.equal(pieces[1].startMinute,0);
  assert.equal(pieces.reduce((n,s)=>n+s.seconds,0),1800);
  assert.equal(splitEvents([event(2,new Date(2026,8,19,23,50),30)],start)[0].seconds,1200);
  assert.deepEqual(visibleHours(pieces),[0,24]);
});

test('overlap groups get lanes; adjacent ordinary rounds share a lane', () => {
  const week = new Date(2026,8,20);
  const data = [event(1,new Date(2026,8,21,9),60),event(2,new Date(2026,8,21,9,15),30),event(3,new Date(2026,8,21,10),25)];
  const pieces = layoutSegments(splitEvents(data,week));
  assert.equal(pieces[0].columns,2);
  assert.notEqual(pieces[0].column,pieces[1].column);
  assert.equal(pieces[2].columns,1);
});

test('DST week boundaries follow calendar dates instead of fixed 168 hours', () => {
  const previous = process.env.TZ;
  process.env.TZ = 'America/New_York';
  try {
    const spring = new Date(2026,2,8);
    const fall = new Date(2026,10,1);
    assert.equal((addDays(spring,7)-spring)/3600000,167);
    assert.equal((addDays(fall,7)-fall)/3600000,169);
    const pieces=splitEvents([event(1,new Date(2026,10,1,0,30),180)],fall);
    assert.equal(pieces.reduce((n,s)=>n+s.seconds,0),10800);
  } finally { if(previous===undefined) delete process.env.TZ; else process.env.TZ=previous; }
});

test('empty/invalid data gives a stable empty grid',()=>{
  assert.deepEqual(splitEvents([event(1,new Date(),-5)],startOfWeek()),[]);
  assert.deepEqual(visibleHours([]),[8,22]);
});

test('background never repeats with two or more images',()=>{
  for (let length=2;length<=8;length++) for(let previous=0;previous<length;previous++) for(let n=0;n<100;n++) {
    const next=nextBackgroundIndex(length,previous,n/100);
    assert.notEqual(next,previous); assert.ok(next>=0&&next<length);
  }
  assert.equal(nextBackgroundIndex(1,0),0);
});

test('pause, ticks and duplicate snapshots do not advance scenery',()=>{
  const prev={round_type:'work',session_work_count:2,work_round_number:2,elapsed_secs:10};
  assert.equal(didAdvanceRound(prev,{...prev,elapsed_secs:11}),false);
  assert.equal(didAdvanceRound(prev,{...prev,is_paused:true}),false);
  assert.equal(didAdvanceRound(prev,{...prev,round_type:'short-break'}),true);
  assert.equal(didAdvanceRound(prev,{...prev,session_work_count:3,work_round_number:3}),true);
});

test('daily totals split sessions at local midnight and keep per-subject time', () => {
  const start = new Date(2026, 8, 20);
  const events = [
    event(1, new Date(2026, 8, 20, 23, 40), 40, { subject_id: 1 }), // 20 min + 20 min
    event(2, new Date(2026, 8, 22, 9, 0), 30),
    event(3, new Date(2026, 8, 19, 23, 50), 20, { subject_id: 2 }), // starts before the range
  ];
  const days = dailyTotals(events, start, 3);
  assert.deepEqual(days.map(d => d.key), ['2026-09-20', '2026-09-21', '2026-09-22']);
  assert.deepEqual(days.map(d => d.seconds / 60), [30, 20, 30]);
  assert.equal(days[0].bySubject.get(1), 20 * 60);
  assert.equal(days[0].bySubject.get(2), 10 * 60);
  assert.equal(days[2].bySubject.get(null), 30 * 60);
});

test('moving average is a trailing mean', () => {
  assert.deepEqual(movingAverage([60, 0, 30, 90], 2), [60, 30, 15, 60]);
  assert.deepEqual(movingAverage([10, 20, 30], 7), [10, 15, 20]);
});

test('subject shares sort largest first, fold the tail and draw full rings', () => {
  const start = new Date(2026, 8, 20);
  const events = [1, 2, 3, 4, 5, 6, 7].map(id => event(id, new Date(2026, 8, 20, 8 + id), id * 5,
    { subject_id: id, subject_name: `S${id}`, subject_color: '#123456' }));
  const shares = subjectShares(events, dailyTotals(events, start, 1));
  assert.equal(shares.length, 6);
  assert.deepEqual(shares.slice(0, 5).map(s => s.id), [7, 6, 5, 4, 3]);
  assert.equal(shares[5].id, 'other');
  assert.equal(shares[5].seconds, (5 + 10) * 60);
  assert.ok(Math.abs(shares.reduce((a, s) => a + s.fraction, 0) - 1) < 1e-9);
  assert.equal((donutArc(50, 50, 30, 40, 0, 1).match(/M/g) ?? []).length, 2);
});

test('each theme keeps its own background and old values are clamped', () => {
  let all = parseBackgrounds('{"Citrus Club":{"timer":{"path":"C:/a.jpg","opacity":400,"fit":"odd","scale":1}}}');
  assert.deepEqual(backgroundFor(all, 'Citrus Club', 'timer'), { path: 'C:/a.jpg', opacity: 100, fit: 'tile', scale: 25 });
  assert.equal(backgroundFor(all, 'Cherry Soda', 'timer'), null);
  all = withBackground(all, 'Cherry Soda', 'timer', { path: 'C:/b.png', fit: 'cover' });
  assert.equal(backgroundFor(all, 'Cherry Soda', 'timer').fit, 'cover');
  assert.equal(backgroundFor(all, 'Citrus Club', 'timer').path, 'C:/a.jpg');
  all = withBackground(all, 'Citrus Club', 'timer', null);
  assert.equal(all['Citrus Club'], undefined);
  assert.deepEqual(parseBackgrounds('not json'), {});
});

test('pattern repeat is found in a picture cut at an arbitrary size, and not in noise', () => {
  const w = 173, h = 131; // not multiples of the 37 × 29 period
  const motif = (x, y) => (Math.sin((x % 37) / 37 * 2 * Math.PI) + Math.cos((y % 29) / 29 * 2 * Math.PI) > 0.6 ? 0.9 : 0.2);
  const lum = new Float32Array(w * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) lum[y * w + x] = motif(x, y);
  assert.equal(findPeriod(lum, w, h, 'x'), 37);
  assert.equal(findPeriod(lum, w, h, 'y'), 29);
  let seed = 3;
  const noise = Float32Array.from({ length: w * h }, () => (seed = (seed * 16807) % 2147483647) / 2147483647);
  assert.equal(findPeriod(noise, w, h, 'x'), null);
  // A smooth gradient (a photo-like sky) has no repeat either.
  const gradient = Float32Array.from({ length: w * h }, (_, k) => (k % w) / w);
  assert.equal(findPeriod(gradient, w, h, 'x'), null);
});

test('timer layout keeps the themed dial when small and scales up when large', () => {
  assert.deepEqual(timerLayout(360, 478), { compact: false, uiScale: 1 });
  assert.equal(timerLayout(240, 240).compact, true);
  assert.ok(timerLayout(240, 240).uiScale >= 0.4);
  assert.equal(timerLayout(1915, 1035).compact, false);
  assert.ok(Math.abs(timerLayout(1915, 1035).uiScale - 995 / 438) < 1e-9);
  assert.ok(coverTileWidth(736, 1104) >= 360);
});

test('settings UI tokens meet WCAG AA in every theme', () => {
  const ratio = (a, b) => (Math.max(luminance(a), luminance(b)) + .05) / (Math.min(luminance(a), luminance(b)) + .05);
  for (const item of collection) {
    const { colors: c } = JSON.parse(fs.readFileSync(new URL(`../static/themes/${item.id}.json`, import.meta.url), 'utf8'));
    const text = [
      ['--ui-on-brand', '--ui-brand'], ['--ui-on-brand', '--ui-brand-strong'],
      ['--ui-text', '--ui-page'], ['--ui-text', '--ui-surface'], ['--ui-text', '--ui-sidebar'], ['--ui-text', '--ui-hover'],
      ['--ui-text-muted', '--ui-page'], ['--ui-text-muted', '--ui-surface'], ['--ui-text-muted', '--ui-sidebar'],
      ['--ui-brand-strong', '--ui-selected'], ['--ui-brand-strong', '--ui-surface'], ['--ui-brand', '--ui-surface'],
      ['--ui-danger', '--ui-surface'], ['--ui-danger', '--ui-danger-soft'],
    ];
    for (const [fg, bg] of text) assert.ok(ratio(c[fg], c[bg]) >= 4.5, `${item.id} ${fg} on ${bg}: ${ratio(c[fg], c[bg]).toFixed(2)}`);
    // Non-text UI parts (switch track, brand controls) need 3:1 against the card.
    for (const part of ['--ui-switch-off', '--ui-brand']) assert.ok(ratio(c[part], c['--ui-surface']) >= 3, `${item.id} ${part}`);
  }
});
const contrast = (a, b) => (Math.max(luminance(a), luminance(b)) + .05) / (Math.min(luminance(a), luminance(b)) + .05);

test('classic tomato: subject palette is the eight varieties, each with readable text', () => {
  const classic = collection.find(t => t.id === 'classic-tomato');
  assert.deepEqual(classic.subjectColors, VARIETIES.map(v => v.hex));
  assert.equal(new Set(VARIETIES.map(v => v.id)).size, 8);
  for (const v of VARIETIES) assert.ok(contrast(v.ink, v.hex) >= 4.5, `${v.id} ${contrast(v.ink, v.hex).toFixed(2)}`);
  assert.equal(varietyFor('#e9a60c')?.id, 'yellow');
  assert.equal(varietyFor('#123456'), undefined);
});

test('nearest varieties: keeps existing ones, one variety per subject, hue first', () => {
  const subjects = [
    { id: 1, color: '#E9A60C' }, // already golden
    { id: 2, color: '#3B82F6' }, // blue
    { id: 3, color: '#EF4444' }, // red
    { id: 4, color: '#F59E0B' }, // amber (yellow is taken)
    { id: 5, color: 'not-a-color' },
  ];
  const m = nearestVarieties(subjects);
  assert.equal(m.has(1), false);
  assert.equal(m.has(5), false);
  assert.equal(m.get(2).variety.id, 'indigo');
  assert.equal(m.get(3).variety.id, 'red');
  assert.equal(m.get(4).variety.id, 'orange');
  const used = [...m.values()].map(x => x.variety.id);
  assert.equal(new Set(used).size, used.length);
  // More subjects than free varieties: extras reuse the nearest and say so.
  const many = Array.from({ length: 10 }, (_, i) => ({ id: i, color: ['#3B82F6', '#8B5CF6', '#EF4444', '#F59E0B', '#EC4899', '#10B981', '#6B7280', '#0EA5E9', '#84CC16', '#F97316'][i] }));
  const all = nearestVarieties(many);
  assert.equal(all.size, 10);
  assert.equal([...all.values()].filter(x => x.shared).length, 2);
  assert.equal(new Set([...all.values()].filter(x => !x.shared).map(x => x.variety.id)).size, 8);
});

test('drum scale: at most 6 numbers and 60 fine ticks for every duration', () => {
  assert.deepEqual(drumScale(25).labels, [0, 5, 10, 15, 20]);
  assert.equal(drumScale(25).fine, 0.5);
  assert.deepEqual(drumScale(90).labels, [0, 15, 30, 45, 60, 75]);
  assert.deepEqual(drumScale(35).labels, [0, 10, 20, 30]);
  assert.deepEqual(drumScale(5).labels, [0, 1, 2, 3, 4]);
  for (let T = 1; T <= 90; T++) {
    const s = drumScale(T);
    assert.ok(s.labels.length <= 6, `T=${T}`);
    assert.ok(Math.round(T / s.fine) <= 60, `T=${T}`);
    if (T >= 4) assert.ok(s.labels.length >= 3, `T=${T}`);
  }
});

test('drum angle: idle 0/T at the pointer, numbers pass right to left while counting down', () => {
  assert.ok(Math.abs(drumAngle(0, 25, 25)) < 1e-9);
  assert.ok(Math.abs(drumAngle(0, 0, 25)) < 1e-9);
  assert.ok(drumAngle(10, 12.5, 25) > 0); // 10 is right of the pointer at 12:30
  assert.ok(drumAngle(15, 12.5, 25) < 0); // 15 is left of it
  assert.ok(Math.abs(Math.abs(drumAngle(0, 12.5, 25)) - Math.PI) < 1e-9); // 0 is on the far side
});

test('typed durations follow the settings rules (1–90 minutes)', () => {
  assert.equal(parseDuration('30'), 1800);
  assert.equal(parseDuration('7:30'), 450);
  assert.equal(parseDuration('30：00'), 1800);
  assert.equal(parseDuration('200'), 5400);
  assert.equal(parseDuration('0'), null);
  assert.equal(parseDuration('12:75'), null);
  assert.equal(parseDuration('abc'), null);
  assert.equal(durationKey('work'), 'time_work_secs');
  assert.equal(durationKey('short-break'), 'time_short_break_secs');
  assert.equal(durationKey('long-break'), 'time_long_break_secs');
});

test('calendar: free space above a block is measured within its day', () => {
  const week = new Date(2026, 8, 20);
  const pieces = layoutSegments(splitEvents([event(1, new Date(2026, 8, 21, 9), 25), event(2, new Date(2026, 8, 21, 9, 30), 25), event(3, new Date(2026, 8, 22, 9, 30), 25)], week));
  const [a, b, c] = pieces.sort((x, y) => x.event.id - y.event.id);
  assert.equal(minutesFreeAbove(pieces, a), Infinity);
  assert.ok(Math.abs(minutesFreeAbove(pieces, b) - 5) < 1e-9);
  assert.equal(minutesFreeAbove(pieces, c), Infinity);
});

test('classic tomato: every subject gets a variety, older colors their closest free one', () => {
  const map = subjectVarieties([{ id: 1, color: '#E9A60C' }, { id: 2, color: '#F5E27A' }, { id: 3, color: '#A94F55' }, { id: 4, color: '#0F6B6F' }]);
  assert.equal(map.get(1).id, 'yellow');
  assert.equal(map.size, 4);
  assert.equal(new Set([...map.values()].map(v => v.id)).size, 4);
});

test('calendar: consecutive rounds of one subject on one day merge into one block', () => {
  const at = (h, m, day = 21) => new Date(2026, 8, day, h, m);
  const math = { subject_id: 1, subject_name: 'Math' };
  const blocks = mergeRounds([
    event(3, at(9, 30), 25, { ...math, task_title: 'Limits' }),
    event(1, at(8, 30), 25, { ...math, task_title: 'Limits' }),
    event(2, at(9, 0), 25, { ...math, task_title: 'Series' }),   // 5-min gap: same block
    event(4, at(10, 5), 25, math),                              // 10-min gap: still the same block
    event(5, at(10, 41), 25, math),                             // 11-min gap: new block
    event(6, at(11, 10), 25, { subject_id: 2, subject_name: 'English' }), // other subject: new block
    event(7, at(23, 40), 25, math), event(8, at(0, 10, 22), 25, math),    // across midnight: two days
  ]);
  assert.deepEqual(blocks.map(b => [b.id, b.rounds]), [[1, 4], [5, 1], [6, 1], [7, 1], [8, 1]]);
  const first = blocks[0];
  assert.equal(first.duration_secs, (10 * 60 + 30 - (8 * 60 + 30)) * 60); // 08:30 to 10:30
  assert.equal(first.focus_secs, 4 * 25 * 60);
  assert.deepEqual(first.tasks, ['Limits', 'Series']);
  assert.equal(first.task_title, null); // several tasks: the block shows the subject
  assert.equal(mergeRounds([event(1, at(8, 0), 25, { ...math, task_title: 'Limits' }), event(2, at(8, 30), 25, math)])[0].task_title, 'Limits');
  assert.deepEqual(mergeRounds([event(1, at(8, 0), 25), event(2, at(8, 30), 25)]).map(b => b.rounds), [2]); // uncategorized merges too
  assert.deepEqual(first.members.map(m => m.id), [1, 2, 3, 4]);
});

test('calendar: weeks can start on Sunday or Monday', () => {
  const sunday = new Date(2026, 8, 27, 15, 0);
  const wednesday = new Date(2026, 8, 30, 8, 0);
  assert.equal(dateKey(startOfWeek(sunday)), '2026-09-27');
  assert.equal(dateKey(startOfWeek(sunday, true)), '2026-09-21'); // a Sunday ends the Monday week
  assert.equal(dateKey(startOfWeek(wednesday, true)), '2026-09-28');
  assert.deepEqual([dayOfWeek(sunday), dayOfWeek(sunday, true), dayOfWeek(wednesday, true)], [0, 6, 2]);
});

test('calendar: an unfinished round adds its focus to the block but is not a round', () => {
  const at = (h, m) => new Date(2026, 8, 21, h, m);
  const [block] = mergeRounds([event(1, at(8, 0), 25), event(2, at(8, 30), 10, { completed: false })]);
  assert.deepEqual([block.rounds, block.focus_secs, block.members.length], [1, 35 * 60, 2]);
  assert.equal(mergeRounds([event(1, at(8, 0), 10, { completed: false })])[0].rounds, 0);
});

test('calendar editing: dragged blocks move every round, edges change the last one', () => {
  const at = (h, m, day = 21) => new Date(2026, 8, day, h, m);
  const secs = (d) => d.getTime() / 1000;
  const math = { subject_id: 1, task_id: 7 };
  const [block] = mergeRounds([event(1, at(8, 30), 25, math), event(2, at(9, 0), 25, math)]);
  // One day later and 45 minutes earlier; the 5-minute gap between the rounds stays.
  const moved = movedRecords(block, 1, -45);
  assert.deepEqual(moved.map(([round, r]) => [round.id, r.started_at]), [[1, secs(at(7, 45, 22))], [2, secs(at(8, 15, 22))]]);
  assert.deepEqual(moved.map(([, r]) => [r.duration_secs, r.subject_id, r.task_id]), [[1500, 1, 7], [1500, 1, 7]]);
  // Lower edge: the last round grows or shrinks, never below five minutes or above twelve hours.
  assert.deepEqual(resizedRecord(block, 20).map(x => x.id ?? x.duration_secs), [2, 45 * 60]);
  assert.equal(resizedRecord(block, -60)[1].duration_secs, 5 * 60);
  assert.equal(resizedRecord(block, 24 * 60)[1].duration_secs, 12 * 3600);
  // Wall-clock helpers used by drawing and dragging.
  assert.equal(atMinute(at(0, 0), 14 * 60 + 5), secs(at(14, 5)));
  assert.equal(atMinute(at(0, 0), 24 * 60), secs(at(0, 0, 22)));
  assert.equal(shiftStart(secs(at(23, 50)), 0, 20), secs(at(0, 10, 22)));
  assert.deepEqual([7, 8, 12.4, -3].map(m => snapMinutes(m)), [5, 10, 10, -5]);
  // A day added across the autumn DST change keeps the wall-clock time (where the zone has one).
  const beforeDst = new Date(2026, 9, 24, 9, 0);
  assert.equal(new Date(shiftStart(secs(beforeDst), 1, 0) * 1000).getHours(), 9);
});

test('jots: open newest first, a just-crossed-off jot stays in place, handled by when', () => {
  const jot = (id, created_at, done_at = null, task_id = null) => ({ id, body: `j${id}`, created_at, done_at, task_id, subject_id: null, in_focus: false });
  const list = [jot(1, 100), jot(2, 300, 900), jot(3, 200), jot(4, 50, 950, 7), jot(5, 400, 800)];
  const { open, handled } = splitJots(list, new Set([5]));
  assert.deepEqual(open.map(j => j.id), [5, 3, 1]);
  assert.deepEqual(handled.map(j => j.id), [4, 2]);
  assert.deepEqual(splitJots(list, new Set()).open.map(j => j.id), [3, 1]);
});

test('jots: the list folds only while a focus round is running', () => {
  assert.equal(foldsInFocus({ round_type: 'work', is_running: true }), true);
  assert.equal(foldsInFocus({ round_type: 'work', is_running: false }), false);
  assert.equal(foldsInFocus({ round_type: 'short-break', is_running: true }), false);
  assert.equal(foldsInFocus({ round_type: 'long-break', is_running: true }), false);
});

test('jots: when-labels read like a notebook', () => {
  const now = new Date(2026, 8, 28, 20, 15);
  const at = (d, h, m, y = 2026, mo = 8) => new Date(y, mo, d, h, m).getTime() / 1000;
  assert.equal(jotWhen(at(28, 9, 5), now, true), '09:05');
  assert.equal(jotWhen(at(27, 23, 59), now, true), '昨天 23:59');
  assert.equal(jotWhen(at(27, 23, 59), now, false), 'Yesterday 23:59');
  assert.equal(jotWhen(at(3, 8, 0), now, true), '9月3日');
  assert.equal(jotWhen(at(3, 8, 0), now, false), 'Sep 3');
  assert.equal(jotWhen(at(30, 8, 0, 2025, 11), now, true), '2025年12月30日');
  assert.equal(jotWhen(at(30, 8, 0, 2025, 11), now, false), 'Dec 30, 2025');
});

test('jots: badge and Enter (an IME candidate confirmation never sends)', () => {
  assert.deepEqual([0, 1, 9, 10, 42].map(badgeText), ['', '1', '9', '9+', '9+']);
  const key = (over) => ({ key: 'Enter', shiftKey: false, isComposing: false, keyCode: 13, ...over });
  assert.equal(isSendKey(key({})), true);
  assert.equal(isSendKey(key({ shiftKey: true })), false);
  assert.equal(isSendKey(key({ isComposing: true })), false);
  assert.equal(isSendKey(key({ keyCode: 229 })), false);
  assert.equal(isSendKey(key({ key: 'a', keyCode: 65 })), false);
});
