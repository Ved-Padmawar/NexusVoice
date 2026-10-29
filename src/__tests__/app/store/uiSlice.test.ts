import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { PillSettings, PillSettingsSnapshot } from '../../../shared/bindings'

const { commands, listener } = vi.hoisted(() => ({
  commands: { setPillSettings: vi.fn(), getPillSettings: vi.fn(), logFrontend: vi.fn(() => Promise.resolve(null)) },
  listener: { current: null as ((e: { payload: PillSettingsSnapshot }) => void) | null },
}))

vi.mock('../../../shared/bindings', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../shared/bindings')>()),
  commands,
  events: {
    pillSettingsChanged: {
      listen: vi.fn((cb: typeof listener.current) => {
        listener.current = cb
        return Promise.resolve(() => {})
      }),
    },
  },
}))

const BASE: PillSettings = { theme: 'steel', waveformStyle: 'bars', liveTranscript: false, startSound: false }
const snapshot = (revision: number, change: Partial<PillSettings>): PillSettingsSnapshot => ({
  revision,
  settings: { ...BASE, ...change },
})

// The store keeps its save bookkeeping per instance, so each test loads its own.
async function loadStore() {
  const { useAppStore } = await import('../../../app/store/useAppStore')
  useAppStore.setState({ pill: BASE })
  return useAppStore
}

beforeEach(() => {
  vi.resetModules()
  vi.clearAllMocks()
  listener.current = null
  commands.getPillSettings.mockResolvedValue(snapshot(0, {}))
})

describe('updatePill', () => {
  it('keeps newer edits when delayed snapshots arrive, during and after the saves', async () => {
    const saves: Array<(s: PillSettingsSnapshot) => void> = []
    commands.setPillSettings.mockImplementation(
      () => new Promise<PillSettingsSnapshot>((resolve) => saves.push(resolve)),
    )
    const store = await loadStore()
    const stop = store.getState().followPillSettings()
    await vi.waitFor(() => expect(listener.current).not.toBeNull())

    const first = store.getState().updatePill({ theme: 'dawn' })
    const second = store.getState().updatePill({ startSound: true })
    listener.current?.({ payload: snapshot(1, { theme: 'dawn' }) })
    expect(store.getState().pill).toEqual({ ...BASE, theme: 'dawn', startSound: true })

    saves[0](snapshot(1, { theme: 'dawn' }))
    saves[1](snapshot(2, { theme: 'dawn', startSound: true }))
    await Promise.all([first, second])

    listener.current?.({ payload: snapshot(1, { theme: 'dawn' }) })
    expect(store.getState().pill).toEqual({ ...BASE, theme: 'dawn', startSound: true })

    commands.setPillSettings.mockResolvedValue(snapshot(3, {}))
    await store.getState().updatePill({ theme: 'aurora' })
    expect(commands.setPillSettings).toHaveBeenLastCalledWith({ ...BASE, theme: 'aurora', startSound: true })
    stop()
  })

  it('takes the backend copy when the last save fails', async () => {
    commands.setPillSettings.mockRejectedValue(new Error('disk full'))
    commands.getPillSettings.mockResolvedValue(snapshot(3, { theme: 'aurora' }))
    const store = await loadStore()

    await store.getState().updatePill({ theme: 'dawn' })

    expect(store.getState().pill).toEqual({ ...BASE, theme: 'aurora' })
  })
})
