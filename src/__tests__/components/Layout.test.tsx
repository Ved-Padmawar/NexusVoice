import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter, Routes, Route } from 'react-router'
import { Layout } from '../../components/Layout'
import { useAppStore } from '../../store/useAppStore'
import { ROUTES } from '../../lib/routes'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/app', () => ({ getName: vi.fn().mockResolvedValue('NexusVoice') }))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ minimize: vi.fn(), toggleMaximize: vi.fn(), close: vi.fn() }),
}))

const renderAt = (path: string) =>
  render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path={ROUTES.DASHBOARD} element={<Layout />}>
          <Route index element={<div>dashboard page</div>} />
          <Route path={ROUTES.SETTINGS.slice(1)} element={<div>settings page</div>} />
          <Route path={ROUTES.DICTIONARY.slice(1)} element={<div>dictionary page</div>} />
        </Route>
      </Routes>
    </MemoryRouter>,
  )

beforeEach(() => {
  useAppStore.setState({
    hasHotkey: true,
    modelReady: true,
    activeModelName: 'Whisper Small',
    updateStatus: 'idle',
    updateDismissed: false,
  })
})

describe('Layout — dock', () => {
  it('offers every destination and marks the current one', () => {
    renderAt(ROUTES.DICTIONARY)
    const nav = screen.getByRole('navigation', { name: /primary/i })
    for (const label of ['Dashboard', 'Dictionary', 'Settings']) {
      expect(screen.getByRole('link', { name: label })).toBeInTheDocument()
    }
    expect(nav.querySelector('[aria-current="page"]')).toHaveAccessibleName('Dictionary')
  })

  // The capsule only morphs if the others stay bare glyphs.
  it('spells out the current destination and no other', () => {
    renderAt(ROUTES.DICTIONARY)
    const nav = screen.getByRole('navigation', { name: /primary/i })
    expect(nav).toHaveTextContent('Dictionary')
    expect(nav).not.toHaveTextContent('Dashboard')
    expect(nav).not.toHaveTextContent('Settings')
  })
})

describe('Layout — update card', () => {
  // Floating over the page now, so its visibility rule is all that hides it.
  it('stays hidden until an update is available', () => {
    renderAt(ROUTES.DASHBOARD)
    expect(screen.queryByRole('button', { name: /install update/i })).not.toBeInTheDocument()
  })

  it('offers the install in place once one is available', () => {
    useAppStore.setState({ updateStatus: 'available', updateVersion: '9.9.9' })
    renderAt(ROUTES.DASHBOARD)
    expect(screen.getByRole('button', { name: /install update/i })).toBeInTheDocument()
    expect(screen.getByText(/9\.9\.9/)).toBeInTheDocument()
  })
})

describe('Layout — status lamp', () => {
  it('reads Ready when a model is loaded and a hotkey is set', () => {
    renderAt(ROUTES.DASHBOARD)
    expect(screen.getByText('Ready')).toBeInTheDocument()
  })

  it('flags a missing hotkey as an action', () => {
    useAppStore.setState({ hasHotkey: false, modelReady: true })
    renderAt(ROUTES.DASHBOARD)
    expect(screen.getByRole('button', { name: /no hotkey/i })).toBeInTheDocument()
  })

  it('flags a missing model ahead of a missing hotkey', () => {
    useAppStore.setState({ hasHotkey: false, modelReady: false })
    renderAt(ROUTES.DASHBOARD)
    expect(screen.getByRole('button', { name: /no model/i })).toBeInTheDocument()
  })
})
