import { createElement } from 'react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook } from '@testing-library/react'
import { QueryClientProvider } from '@tanstack/react-query'
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

// The save bookkeeping is module state, so each test loads its own copy.
async function load() {
  const pill = await import('../../../features/settings/queries')
  const { queryClient } = await import('../../../app/queryClient')
  const { queryKeys } = await import('../../../shared/queryKeys')
  queryClient.setQueryData(queryKeys.pillSettings, BASE)
  return { pill, current: () => queryClient.getQueryData<PillSettings>(queryKeys.pillSettings) }
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
    const { pill, current } = await load()
    const stop = pill.followPillSettings()
    await vi.waitFor(() => expect(listener.current).not.toBeNull())

    const first = pill.updatePill({ theme: 'dawn' })
    const second = pill.updatePill({ startSound: true })
    listener.current?.({ payload: snapshot(1, { theme: 'dawn' }) })
    expect(current()).toEqual({ ...BASE, theme: 'dawn', startSound: true })

    saves[0](snapshot(1, { theme: 'dawn' }))
    saves[1](snapshot(2, { theme: 'dawn', startSound: true }))
    await Promise.all([first, second])

    listener.current?.({ payload: snapshot(1, { theme: 'dawn' }) })
    expect(current()).toEqual({ ...BASE, theme: 'dawn', startSound: true })

    commands.setPillSettings.mockResolvedValue(snapshot(3, {}))
    await pill.updatePill({ theme: 'aurora' })
    expect(commands.setPillSettings).toHaveBeenLastCalledWith({ ...BASE, theme: 'aurora', startSound: true })
    stop()
  })

  it('keeps a pending edit when a read returns the older settings', async () => {
    let finishSave!: (s: PillSettingsSnapshot) => void
    commands.setPillSettings.mockImplementationOnce(
      () => new Promise<PillSettingsSnapshot>((resolve) => { finishSave = resolve }),
    )
    const { pill, current } = await load()
    const { queryClient } = await import('../../../app/queryClient')
    const { queryKeys } = await import('../../../shared/queryKeys')
    renderHook(() => pill.usePillSettings(), {
      wrapper: ({ children }) => createElement(QueryClientProvider, { client: queryClient }, children),
    })

    const first = pill.updatePill({ theme: 'dawn' })
    await queryClient.refetchQueries({ queryKey: queryKeys.pillSettings })
    expect(current()).toEqual({ ...BASE, theme: 'dawn' })

    commands.setPillSettings.mockResolvedValueOnce(snapshot(2, { theme: 'dawn', startSound: true }))
    await pill.updatePill({ startSound: true })
    expect(commands.setPillSettings).toHaveBeenLastCalledWith({ ...BASE, theme: 'dawn', startSound: true })
    finishSave(snapshot(1, { theme: 'dawn' }))
    await first
  })

  it('takes the backend copy when the last save fails', async () => {
    commands.setPillSettings.mockRejectedValue(new Error('disk full'))
    commands.getPillSettings.mockResolvedValue(snapshot(3, { theme: 'aurora' }))
    const { pill, current } = await load()

    await pill.updatePill({ theme: 'dawn' })

    expect(current()).toEqual({ ...BASE, theme: 'aurora' })
  })
})
