import { describe, it, expect, vi, beforeEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore } from '../../store/useAppStore'
import { queryClient, queryKeys, NO_FILTERS, type TranscriptPages } from '../../lib/queries'

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
  queryClient.getQueryData<TranscriptPages>(queryKeys.transcripts(NO_FILTERS))?.pages.flat() ?? []

beforeEach(() => {
  mockInvoke.mockReset()
  queryClient.clear()
  useAppStore.setState({
    starting: false,
    startupError: null,
    hasHotkey: false,
    modelReady: false,
    downloads: {},
  })
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

describe('useAppStore — model downloads', () => {
  it('marks a model queued as soon as a download starts', async () => {
    mockInvoke.mockResolvedValue(undefined)
    await useAppStore.getState().startDownload('whisper-tiny')
    expect(useAppStore.getState().downloads['whisper-tiny']).toEqual({
      status: 'queued',
      progress: 0,
    })
    expect(mockInvoke).toHaveBeenCalledWith('start_model_download', { id: 'whisper-tiny' })
  })

  it('drops the entry when the backend refuses the download', async () => {
    mockInvoke.mockRejectedValue(new Error('unknown model'))
    await useAppStore.getState().startDownload('nope')
    expect(useAppStore.getState().downloads.nope).toBeUndefined()
  })

  it('tracks several downloads independently', async () => {
    mockInvoke.mockResolvedValue(undefined)
    await useAppStore.getState().startDownload('a')
    await useAppStore.getState().startDownload('b')
    expect(Object.keys(useAppStore.getState().downloads).sort()).toEqual(['a', 'b'])
  })

  it('rehydrates in-flight downloads from the backend', async () => {
    mockInvoke.mockResolvedValue([
      { id: 'a', status: 'running', progress: 40, error: null },
      { id: 'b', status: 'queued', progress: 0, error: null },
    ])
    await useAppStore.getState().refreshDownloads()
    expect(useAppStore.getState().downloads).toEqual({
      a: { status: 'running', progress: 40, error: null },
      b: { status: 'queued', progress: 0, error: null },
    })
  })

  it('cancels by id', async () => {
    mockInvoke.mockResolvedValue(true)
    await useAppStore.getState().cancelDownload('whisper-tiny')
    expect(mockInvoke).toHaveBeenCalledWith('cancel_model_download', { id: 'whisper-tiny' })
  })

  it('never switches the active model as a side effect of downloading', async () => {
    // Download and switch are separate actions — a download that fails or is
    // cancelled must not leave the override pointing at a missing file.
    mockInvoke.mockResolvedValue(undefined)
    useAppStore.setState({ selectedModel: 'whisper-tiny' })
    await useAppStore.getState().startDownload('parakeet-v3')
    expect(useAppStore.getState().selectedModel).toBe('whisper-tiny')
    expect(mockInvoke).not.toHaveBeenCalledWith('set_model_override', expect.anything())
  })
})
