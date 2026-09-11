import { describe, it, expect } from 'vitest'
import { fmtTime, fmtDate } from '../../lib/utils'

describe('fmtTime', () => {
  it('reports sub-minute durations in seconds', () => {
    expect(fmtTime(0)).toBe('0s')
    expect(fmtTime(1)).toBe('1s')
    expect(fmtTime(59)).toBe('59s')
  })

  it('switches to minutes at exactly 60 seconds', () => {
    expect(fmtTime(60)).toBe('1m')
  })

  it('keeps the leftover seconds alongside the minutes', () => {
    expect(fmtTime(61)).toBe('1m 1s')
    expect(fmtTime(125)).toBe('2m 5s')
  })

  it('omits a zero seconds remainder', () => {
    // "5m 0s" reads as broken formatting, not as a precise duration.
    expect(fmtTime(300)).toBe('5m')
  })

  it('switches to hours at exactly 3600 seconds', () => {
    expect(fmtTime(3600)).toBe('1h')
  })

  it('keeps the leftover minutes alongside the hours and drops the seconds', () => {
    // Hour-scale totals round to the minute; the seconds are noise there.
    expect(fmtTime(3660)).toBe('1h 1m')
    expect(fmtTime(3719)).toBe('1h 1m')
    expect(fmtTime(7200)).toBe('2h')
  })

  it('handles a multi-hour total', () => {
    expect(fmtTime(36_000)).toBe('10h')
    expect(fmtTime(45_296)).toBe('12h 34m')
  })
})

describe('fmtDate', () => {
  it('formats a valid ISO timestamp', () => {
    // Locale-dependent, so assert the parts rather than an exact string.
    const out = fmtDate('2026-03-15T14:30:00Z')
    expect(out).toMatch(/Mar/)
    expect(out).toMatch(/15/)
    expect(out).not.toBe('Invalid Date')
  })

  it('reports an unparseable timestamp rather than throwing', () => {
    // A corrupt created_at must not crash the transcript list.
    expect(fmtDate('not a date')).toBe('Invalid Date')
    expect(fmtDate('')).toBe('Invalid Date')
  })

  it('is stable across calls for the same input', () => {
    // The Intl formatter is created once at module scope; reuse must not
    // accumulate state or drift.
    const input = '2026-03-15T14:30:00Z'
    expect(fmtDate(input)).toBe(fmtDate(input))
  })

  it('accepts the space-separated shape SQLite returns', () => {
    // `created_at` comes back as "YYYY-MM-DD HH:MM:SS", not strict ISO.
    expect(fmtDate('2026-03-15 14:30:00')).not.toBe('Invalid Date')
  })
})
