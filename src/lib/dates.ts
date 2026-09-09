/** Local calendar-day helpers. Kept out of the component file so a fast-refresh
 *  boundary is not broken by exporting non-components alongside one. */

/** Local YYYY-MM-DD. `toISOString` converts to UTC first, which shifts the day
 *  backwards for anyone west of Greenwich. */
export function isoDay(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

export function daysAgo(n: number): string {
  const d = new Date()
  d.setDate(d.getDate() - n)
  return isoDay(d)
}

export function parseDay(s: string | null): Date | null {
  if (!s) return null
  const [y, m, d] = s.split('-').map(Number)
  return y && m && d ? new Date(y, m - 1, d) : null
}

/** Short label for one end of a span, e.g. "Sep 10". */
export function dayLabel(iso: string): string {
  const d = parseDay(iso)
  return d ? d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' }) : iso
}

/**
 * The `from`/`to` bounds the backend compares against `created_at`.
 *
 * Two things have to be reconciled. `created_at` is SQLite `CURRENT_TIMESTAMP`,
 * which is `YYYY-MM-DD HH:MM:SS` in **UTC**, and the query compares it as text.
 * So a bare `to` of "2026-09-09" sorts before "2026-09-09 04:12:00" and excludes
 * everything recorded that day, and a local calendar day is not a UTC one.
 *
 * Both ends are therefore converted from local midnight to a UTC timestamp of
 * the same text shape the column uses.
 */
export function toUtcBounds(from: string | null, to: string | null): {
  from: string | null
  to: string | null
} {
  return { from: startBound(from), to: endBound(to) }
}

const utcStamp = (d: Date) =>
  `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, '0')}-${String(d.getUTCDate()).padStart(2, '0')}` +
  ` ${String(d.getUTCHours()).padStart(2, '0')}:${String(d.getUTCMinutes()).padStart(2, '0')}:${String(d.getUTCSeconds()).padStart(2, '0')}`

/** Local 00:00:00 of `iso`, as UTC. */
function startBound(iso: string | null): string | null {
  const d = parseDay(iso)
  return d ? utcStamp(d) : null
}

/** Local 23:59:59 of `iso`, as UTC — inclusive of the whole day. */
function endBound(iso: string | null): string | null {
  const d = parseDay(iso)
  if (!d) return null
  d.setHours(23, 59, 59, 999)
  return utcStamp(d)
}
