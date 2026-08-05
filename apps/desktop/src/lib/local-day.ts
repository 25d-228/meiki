import { SvelteDate } from "svelte/reactivity";

export function localDayBounds(
  now: Date,
  boundaryMinutes: number,
): { start: SvelteDate; end: SvelteDate } {
  const start = new SvelteDate(now);
  start.setHours(0, boundaryMinutes, 0, 0);
  if (now.getTime() < start.getTime()) start.setDate(start.getDate() - 1);
  const end = new SvelteDate(start);
  end.setDate(end.getDate() + 1);
  return { start, end };
}
