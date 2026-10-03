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

const SQLITE_UTC = /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/

/** SQLite's CURRENT_TIMESTAMP is UTC with no zone, which `new Date` reads as
 *  local time. Fixed here, not in Rust: the raw string is also the pagination cursor. */
export function fmtDate(d: string): string {
  try {
    const date = new Date(SQLITE_UTC.test(d) ? `${d.replace(' ', 'T')}Z` : d)
    return Number.isNaN(date.getTime()) ? 'Invalid Date' : transcriptDateFormat.format(date)
  } catch { return d }
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
