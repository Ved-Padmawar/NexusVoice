import type { ReactNode } from 'react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClientProvider } from '@tanstack/react-query'
import { invoke } from '@tauri-apps/api/core'
import { queryClient } from '../../../app/queryClient'
import {
  cancelDownload, refreshActiveModel, startDownload, useActiveModel, useDownload,
} from '../../../features/models/queries'
import type { CatalogModel } from '../../../features/models/models'
import { queryKeys } from '../../../shared/queryKeys'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

const mockInvoke = vi.mocked(invoke)

const downloads = () => queryClient.getQueryData<Record<string, unknown>>(queryKeys.downloads)

const wrapper = ({ children }: { children: ReactNode }) =>
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>

beforeEach(() => {
  mockInvoke.mockReset()
  queryClient.clear()
  queryClient.setQueryData(queryKeys.downloads, {})
})

describe('model downloads', () => {
  it('marks a model queued as soon as a download starts', async () => {
    mockInvoke.mockResolvedValue(undefined)
    await startDownload('whisper-tiny')
    expect(downloads()?.['whisper-tiny']).toEqual({ status: 'queued', progress: 0 })
    expect(mockInvoke).toHaveBeenCalledWith('start_model_download', { id: 'whisper-tiny' })
  })

  it('drops the entry when the backend refuses the download', async () => {
    mockInvoke.mockRejectedValue(new Error('unknown model'))
    await startDownload('nope')
    expect(downloads()?.nope).toBeUndefined()
  })

  it('tracks several downloads independently', async () => {
    mockInvoke.mockResolvedValue(undefined)
    await startDownload('a')
    await startDownload('b')
    expect(Object.keys(downloads() ?? {}).sort()).toEqual(['a', 'b'])
  })

  it('reads in-flight downloads from the backend', async () => {
    queryClient.clear()
    mockInvoke.mockResolvedValue([
      { id: 'a', status: 'running', progress: 40, error: null },
      { id: 'b', status: 'queued', progress: 0, error: null },
    ])
    const { result } = renderHook(() => useDownload('b'), { wrapper })
    await waitFor(() => expect(result.current.data).toEqual({ status: 'queued', progress: 0, error: null }))
    expect(downloads()).toHaveProperty('a')
  })

  it('cancels by id', async () => {
    mockInvoke.mockResolvedValue(true)
    await cancelDownload('whisper-tiny')
    expect(mockInvoke).toHaveBeenCalledWith('cancel_model_download', { id: 'whisper-tiny' })
  })

  it('never switches the active model as a side effect of downloading', async () => {
    // Download and switch are separate actions — a download that fails or is
    // cancelled must not leave the override pointing at a missing file.
    mockInvoke.mockResolvedValue(undefined)
    await startDownload('parakeet-v3')
    expect(mockInvoke).not.toHaveBeenCalledWith('set_model_override', expect.anything())
  })
})

describe('reads racing events', () => {
  it('keeps the newer model when two refreshes finish in reverse order', async () => {
    queryClient.clear()
    let finishFirst!: (info: unknown) => void
    mockInvoke
      .mockImplementationOnce(() => new Promise(resolve => { finishFirst = resolve }))
      .mockResolvedValueOnce({ modelName: 'Parakeet V3', downloaded: true, downloading: false })

    const first = refreshActiveModel()
    await refreshActiveModel()
    finishFirst({ modelName: 'Whisper Tiny', downloaded: true, downloading: false })
    await first

    expect(queryClient.getQueryData<{ modelName: string }>(queryKeys.activeModel)?.modelName).toBe('Parakeet V3')
  })

  it('does not bring back a download that ended while a read was in flight', async () => {
    queryClient.clear()
    let finishRead!: (rows: unknown) => void
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_active_downloads') return new Promise(resolve => { finishRead = resolve })
      return Promise.reject(new Error('refused'))
    })

    // Nothing cached, so starting reads the backend; the refusal lands mid-read.
    await startDownload('a')
    await waitFor(() => expect(finishRead).toBeTypeOf('function'))
    finishRead([{ id: 'a', status: 'running', progress: 99, error: null }])

    await waitFor(() => expect(downloads()).toEqual({}))
  })

  it('replays an event onto a read the hook started', async () => {
    queryClient.clear()
    let finishRead!: (rows: unknown) => void
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_active_downloads') return new Promise(resolve => { finishRead = resolve })
      return Promise.reject(new Error('refused'))
    })
    const { result } = renderHook(() => useDownload('a'), { wrapper })
    await waitFor(() => expect(finishRead).toBeTypeOf('function'))

    await startDownload('a')
    finishRead([{ id: 'a', status: 'running', progress: 99, error: null }])

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toBeUndefined()
  })

  it('keeps a download ended while a refresh and a hook read were both in flight', async () => {
    queryClient.clear()
    const finishReads: Array<(rows: unknown) => void> = []
    let starts = 0
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_active_downloads') return new Promise(resolve => { finishReads.push(resolve) })
      starts += 1
      return starts === 1 ? Promise.resolve(undefined) : Promise.reject(new Error('refused'))
    })

    await startDownload('a')
    await waitFor(() => expect(finishReads).toHaveLength(1))
    const { result } = renderHook(() => useDownload('a'), { wrapper })
    await waitFor(() => expect(finishReads).toHaveLength(2))

    // A second start is refused, which drops the download while both reads are out.
    await startDownload('a')
    const running = [{ id: 'a', status: 'running', progress: 99, error: null }]
    finishReads.forEach(finish => finish(running))

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toBeUndefined()
    expect(downloads()).toEqual({})
  })
})

describe('active model', () => {
  it('reads the model fresh after a switch, not from a read already in flight', async () => {
    queryClient.clear()
    let finishOldRead!: (info: unknown) => void
    let reads = 0
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_model_catalog') return Promise.resolve([])
      reads += 1
      return reads === 1
        ? new Promise(resolve => { finishOldRead = resolve })
        : Promise.resolve({ modelName: 'Parakeet V3', downloaded: true, downloading: false })
    })
    // The first read is in flight when the switch lands.
    renderHook(() => useActiveModel(), { wrapper })
    await waitFor(() => expect(reads).toBe(1))

    const fresh = await refreshActiveModel()
    finishOldRead({ modelName: 'Whisper Tiny', downloaded: true, downloading: false })

    expect(fresh.modelName).toBe('Parakeet V3')
    await waitFor(() =>
      expect(queryClient.getQueryData<{ modelName: string }>(queryKeys.activeModel)?.modelName).toBe('Parakeet V3'))
  })

  it('names the catalog id of the model Rust reports', async () => {
    const catalog = [
      { id: 'whisper-tiny', displayName: 'Whisper Tiny' },
      { id: 'parakeet-v3', displayName: 'Parakeet V3' },
    ] as CatalogModel[]
    queryClient.setQueryData(queryKeys.catalog, catalog)
    queryClient.setQueryData(queryKeys.activeModel, { modelName: 'Parakeet V3', downloaded: false, downloading: true })

    const { result } = renderHook(() => useActiveModel(), { wrapper })

    await waitFor(() => expect(result.current.data).toEqual({ id: 'parakeet-v3', downloaded: false }))
  })
})
