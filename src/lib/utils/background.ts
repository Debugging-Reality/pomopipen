import type { TimerState } from '../types';

/** Reset, pause/resume and repeated snapshots are not session transitions. */
export function didAdvanceRound(previous: TimerState, next: TimerState): boolean {
  return previous.round_type !== next.round_type ||
    previous.session_work_count !== next.session_work_count ||
    previous.work_round_number !== next.work_round_number;
}

export function nextBackgroundIndex(length: number, previous: number, random = Math.random()): number {
  if (length <= 1) return 0;
  if (previous < 0 || previous >= length) return Math.min(length - 1, Math.floor(random * length));
  const pick = Math.min(length - 2, Math.floor(random * (length - 1)));
  return pick >= previous ? pick + 1 : pick;
}
