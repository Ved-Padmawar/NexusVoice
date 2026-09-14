import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function fmtTime(s: number): string {
  if (s < 60) return `${s}s`
  const m = Math.floor(s / 60), rem = s % 60
  if (m < 60) return rem > 0 ? `${m}m ${rem}s` : `${m}m`
  const h = Math.floor(m / 60), rm = m % 60
  return rm > 0 ? `${h}h ${rm}m` : `${h}h`
}

// Creating an Intl formatter per row dominated Dashboard render time.
const transcriptDateFormat = new Intl.DateTimeFormat(undefined, {
  month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
})

export function fmtDate(d: string): string {
  try {
    const date = new Date(d)
    return Number.isNaN(date.getTime()) ? 'Invalid Date' : transcriptDateFormat.format(date)
  } catch { return d }
}

const clockFormat = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' })
const weekdayFormat = new Intl.DateTimeFormat(undefined, { weekday: 'long' })
const dayMonthFormat = new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'long' })
const shortDayFormat = new Intl.DateTimeFormat(undefined, { weekday: 'short', day: 'numeric', month: 'long' })
const fullDayFormat = new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'long', year: 'numeric' })

export function fmtClock(d: string): string {
  const date = new Date(d)
  return Number.isNaN(date.getTime()) ? '' : clockFormat.format(date)
}

export type DayGroup = {
  key: string
  /** "Today", "Yesterday", a weekday within the week, else the date. */
  label: string
  /** The date beside a relative label; null when the label is the date. */
  detail: string | null
}

const MS_PER_DAY = 86_400_000

/** The local calendar day a timestamp falls on, named relative to `now`. */
export function dayOf(d: string, now: Date = new Date()): DayGroup {
  const date = new Date(d)
  if (Number.isNaN(date.getTime())) return { key: 'unknown', label: 'Unknown date', detail: null }

  const key = `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`
  const startOf = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime()
  const daysAgo = Math.round((startOf(now) - startOf(date)) / MS_PER_DAY)
  const withWeekday = `${weekdayFormat.format(date)}, ${dayMonthFormat.format(date)}`

  if (daysAgo === 0) return { key, label: 'Today', detail: withWeekday }
  if (daysAgo === 1) return { key, label: 'Yesterday', detail: withWeekday }
  if (daysAgo > 1 && daysAgo < 7) return { key, label: weekdayFormat.format(date), detail: dayMonthFormat.format(date) }
  if (date.getFullYear() === now.getFullYear()) return { key, label: shortDayFormat.format(date), detail: null }
  return { key, label: fullDayFormat.format(date), detail: null }
}

export function downloadBlob(content: string, filename: string, mime: string): void {
  const blob = new Blob([content], { type: mime })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}
