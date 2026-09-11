/**
 * `SETTINGS_TABS` is the registry; `Settings.tsx` hard-codes the same strings in
 * its triggers and panels. CLAUDE.md calls out keeping the three in sync — a
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

  it('gives every registered tab a trigger in Settings.tsx', () => {
    const missing = Object.values(SETTINGS_TABS).filter(
      tab => !settingsSource.includes(`<TabsTrigger value="${tab}"`),
    )
    expect(missing, 'these tabs have no trigger to click').toEqual([])
  })

  it('gives every registered tab a panel in Settings.tsx', () => {
    const missing = Object.values(SETTINGS_TABS).filter(
      tab => !settingsSource.includes(`<TabsContent value="${tab}"`),
    )
    expect(missing, 'these tabs render nothing when selected').toEqual([])
  })

  it('renders the triggers in the registry order', () => {
    // The registry is the spec; a reordered JSX list silently contradicts it.
    const order = Object.values(SETTINGS_TABS).map(tab => ({
      tab,
      at: settingsSource.indexOf(`<TabsTrigger value="${tab}"`),
    }))
    expect(order.every(o => o.at >= 0)).toBe(true)
    const positions = order.map(o => o.at)
    expect(positions, `trigger order was ${order.map(o => o.tab)}`)
      .toEqual([...positions].sort((a, b) => a - b))
  })

  it('has no trigger or panel for a tab that is not registered', () => {
    // A leftover panel from a removed tab is dead code that still renders.
    const registered = new Set<string>(Object.values(SETTINGS_TABS))
    for (const match of settingsSource.matchAll(/<Tabs(?:Trigger|Content) value="([^"]+)"/g)) {
      expect(registered.has(match[1]), `"${match[1]}" is rendered but not registered`).toBe(true)
    }
  })
})
