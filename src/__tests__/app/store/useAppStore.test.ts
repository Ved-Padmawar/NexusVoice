import { describe, it, expect, vi, beforeEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore } from '../../../app/store/useAppStore'
import { queryClient } from '../../../app/queryClient'
import { queryKeys } from '../../../shared/queryKeys'
import { NO_FILTERS, type TranscriptPages } from '../../../features/history/queries'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }))
vi.mock('sonner', () => ({ toast: { error: vi.fn(), success: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

const page = (start: number, count: number) =>
  Array.from({ length: count }, (_, i) => ({
    id: start + i + 1,
    content: `t${start + i}`,
    wordCount: 1,
    durationSeconds: null,
    targetApp: null,
    createdAt: `2026-01-01T00:00:${String(start + i + 1).padStart(2, '0')}`,
  }))

const feedRows = () =>
  queryClient.getQueryData<TranscriptPages>(queryKeys.transcripts(NO_FILTERS))?.pages.flatMap(page => page.rows) ?? []

beforeEach(() => {
  mockInvoke.mockReset()
  queryClient.clear()
  useAppStore.setState({ starting: false, startupError: null })
})

describe('useAppStore — theme', () => {
  it('setTheme updates theme', () => {
    useAppStore.getState().setTheme('midnight')
    expect(useAppStore.getState().theme).toBe('midnight')
  })
})

describe('useAppStore — startup', () => {
  it('coalesces concurrent starts and waits for initial data before revealing the page', async () => {
    let finishTranscripts!: (value: never[]) => void
    mockInvoke.mockImplementation(cmd => cmd === 'get_transcripts'
      ? new Promise(resolve => { finishTranscripts = resolve })
      : Promise.resolve([]))
    useAppStore.setState({ starting: true })
    const first = useAppStore.getState().startup()
    const second = useAppStore.getState().startup()
    expect(first).toBe(second)
    await vi.waitFor(() => expect(finishTranscripts).toBeTypeOf('function'))
    expect(useAppStore.getState().starting).toBe(true)
    for (const command of ['wait_for_app_ready', 'get_transcripts', 'get_usage_stats', 'get_dictionary']) {
      expect(mockInvoke.mock.calls.filter(([cmd]) => cmd === command)).toHaveLength(1)
    }
    finishTranscripts([])
    await first
    expect(useAppStore.getState().starting).toBe(false)
  })

  it('loads data once the database is ready', async () => {
    mockInvoke.mockResolvedValue([])
    await useAppStore.getState().startup()
    const state = useAppStore.getState()
    expect(state.starting).toBe(false)
    expect(state.startupError).toBeNull()
    expect(mockInvoke).toHaveBeenCalledWith('wait_for_app_ready')
  })

  it('records the error when the database is unavailable', async () => {
    mockInvoke.mockRejectedValue({ code: 'database_unavailable', message: 'disk is full' })
    await useAppStore.getState().startup()
    const state = useAppStore.getState()
    expect(state.starting).toBe(false)
    expect(state.startupError).toBe('disk is full')
  })

  it('can retry startup after a database failure and clear the previous error', async () => {
    mockInvoke.mockResolvedValue([]).mockRejectedValueOnce({ code: 'database_unavailable', message: 'database locked' })
    await useAppStore.getState().startup()
    expect(useAppStore.getState().startupError).toBe('database locked')

    const transcripts = page(0, 2)
    mockInvoke.mockImplementation(cmd => Promise.resolve(cmd === 'get_transcripts' ? transcripts : []))
    await useAppStore.getState().startup()

    expect(mockInvoke.mock.calls.filter(([cmd]) => cmd === 'wait_for_app_ready')).toHaveLength(2)
    expect(useAppStore.getState()).toMatchObject({ starting: false, startupError: null })
    expect(feedRows()).toEqual(transcripts)
  })
})
