/**
 * `SETTINGS_TABS` is the registry; `Settings.tsx` names each entry in its
 * triggers and panels. CLAUDE.md calls out keeping the three in sync — a
 * drift shows up as a tab that renders nothing, with no error.
 */
import { describe, it, expect } from 'vitest'
import { ROUTES, SETTINGS_TABS } from '../../lib/routes'
// Read as text through Vite, so this needs no node APIs and no extra types.
import settingsSource from '../../pages/Settings.tsx?raw'

describe('ROUTES', () => {
  it('roots the dashboard and gives every other route a leading slash', () => {
    expect(ROUTES.DASHBOARD).toBe('/')
    for (const path of Object.values(ROUTES)) expect(path.startsWith('/')).toBe(true)
  })

  it('has no duplicate paths', () => {
    const paths = Object.values(ROUTES)
    expect(new Set(paths).size).toBe(paths.length)
  })
})

describe('SETTINGS_TABS', () => {
  it('keeps the documented tab order', () => {
    // General, Voice, Appearance, About — the registry order *is* the tab order.
    expect(Object.values(SETTINGS_TABS)).toEqual(['general', 'voice', 'appearance', 'about'])
  })

  const keys = Object.keys(SETTINGS_TABS)
  const trigger = (key: string) => `value: SETTINGS_TABS.${key},`
  const panel = (key: string) => `tab === SETTINGS_TABS.${key} &&`

  it('gives every registered tab a trigger in Settings.tsx', () => {
    const missing = keys.filter(key => !settingsSource.includes(trigger(key)))
    expect(missing, 'these tabs have no trigger to click').toEqual([])
  })

  it('gives every registered tab a panel in Settings.tsx', () => {
    const missing = keys.filter(key => !settingsSource.includes(panel(key)))
    expect(missing, 'these tabs render nothing when selected').toEqual([])
  })

  it('renders the triggers in the registry order', () => {
    // The registry is the spec; a reordered list silently contradicts it.
    const positions = keys.map(key => settingsSource.indexOf(trigger(key)))
    expect(positions.every(at => at >= 0)).toBe(true)
    expect(positions, `trigger order was ${keys}`).toEqual([...positions].sort((a, b) => a - b))
  })

  it('has no trigger or panel for a tab that is not registered', () => {
    // A leftover panel from a removed tab is dead code that still renders.
    for (const match of settingsSource.matchAll(/SETTINGS_TABS\.([A-Z_]+)/g)) {
      expect(keys.includes(match[1]), `"${match[1]}" is rendered but not registered`).toBe(true)
    }
  })
})
