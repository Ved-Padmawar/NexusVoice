import { describe, it, expect } from 'vitest'
import { PILL_THEMES, pillThemeDef } from '../../lib/pillThemes'
import type { PillTheme } from '../../store/uiSlice'

/** Every member of the PillTheme union. Listed so adding one to the type
 *  without adding its values here fails a test instead of rendering Steel. */
const ALL_THEMES: PillTheme[] = ['steel', 'midnight', 'canvas', 'dawn']

describe('pillThemeDef', () => {
  it('resolves every theme in the union to its own definition', () => {
    for (const id of ALL_THEMES) {
      expect(pillThemeDef(id).id, `${id} has no definition`).toBe(id)
    }
  })

  it('falls back to the first theme for an unknown id', () => {
    // Persisted state can name a theme a later build removed; rendering Steel
    // beats rendering nothing.
    expect(pillThemeDef('nope' as PillTheme)).toBe(PILL_THEMES[0])
    expect(pillThemeDef(undefined as unknown as PillTheme).id).toBe('steel')
  })

  it('covers the union exactly, with no extra or duplicate entries', () => {
    expect(PILL_THEMES.map(t => t.id).sort()).toEqual([...ALL_THEMES].sort())
    expect(new Set(PILL_THEMES.map(t => t.id)).size).toBe(PILL_THEMES.length)
  })
})

describe('pill theme values', () => {
  it('derives accentRgb as three in-range channels', () => {
    // `accentRgb` is computed from `accent`, so this pins the conversion rather
    // than the two values agreeing (which they now do by construction).
    for (const t of PILL_THEMES) {
      const channels = t.accentRgb.split(',')
      expect(channels, `${t.id}: expected three channels`).toHaveLength(3)
      for (const c of channels) {
        const n = Number(c)
        expect(Number.isInteger(n), `${t.id}: "${c}" is not an integer`).toBe(true)
        expect(n, `${t.id}: channel ${c} out of range`).toBeGreaterThanOrEqual(0)
        expect(n, `${t.id}: channel ${c} out of range`).toBeLessThanOrEqual(255)
      }
    }
    // One known value end-to-end, so a broken parse cannot pass the shape check.
    expect(pillThemeDef('dawn').accentRgb).toBe('228,56,0')
  })

  it('gives every theme a complete, well-formed set of values', () => {
    for (const t of PILL_THEMES) {
      expect(t.label, `${t.id} has no label`).toBeTruthy()
      expect(t.accent, `${t.id} accent`).toMatch(/^#[0-9a-fA-F]{6}$/)
      expect(t.bg, `${t.id} bg`).toMatch(/^#[0-9a-fA-F]{6}$/)
      expect(t.border, `${t.id} border`).toMatch(/^rgba?\(/)
      expect(t.brand, `${t.id} brand`).toMatch(/^rgba?\(/)
    }
  })

  it('keeps light and dark themes visibly distinct from their background', () => {
    // A theme whose accent matches its own background is invisible.
    for (const t of PILL_THEMES) {
      expect(t.accent.toLowerCase(), `${t.id}: accent equals bg`).not.toBe(t.bg.toLowerCase())
    }
  })
})
