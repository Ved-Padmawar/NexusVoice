import { describe, it, expect } from 'vitest'
import { isoDay, daysAgo, toUtcBounds } from '../../lib/dates'

describe('isoDay', () => {
  it('uses the local calendar day, not UTC', () => {
    // 00:30 local on the 9th is still the 9th, even where that is the 8th in UTC.
    const d = new Date(2026, 8, 9, 0, 30)
    expect(isoDay(d)).toBe('2026-09-09')
  })
})

describe('daysAgo', () => {
  it('counts back whole local days', () => {
    expect(daysAgo(0)).toBe(isoDay(new Date()))
  })
})

describe('toUtcBounds', () => {
  it('spans a full local day, so rows recorded that day are included', () => {
    const { from, to } = toUtcBounds('2026-09-09', '2026-09-09')
    expect(from).not.toBeNull()
    expect(to).not.toBeNull()
    // The end bound must sort after any timestamp on that local day; a bare
    // "2026-09-09" would sort before "2026-09-09 04:12:00" and exclude it.
    expect(from! < to!).toBe(true)
    expect(to!).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/)
    expect(from!).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/)
  })

  it('passes nulls through as no bound', () => {
    expect(toUtcBounds(null, null)).toEqual({ from: null, to: null })
  })
})
