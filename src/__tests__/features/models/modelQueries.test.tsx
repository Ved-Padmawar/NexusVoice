import type { ReactNode } from 'react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClientProvider } from '@tanstack/react-query'
import { invoke } from '@tauri-apps/api/core'
import { queryClient } from '../../../app/queryClient'
import { startDownload, useActiveModel } from '../../../features/models/queries'
import type { CatalogModel } from '../../../features/models/models'

const mockInvoke = vi.mocked(invoke)

const wrapper = ({ children }: { children: ReactNode }) =>
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>

beforeEach(() => {
  mockInvoke.mockReset()
  queryClient.clear()
})

describe('model downloads', () => {
  it('asks Rust to switch to a model only once its download lands', async () => {
    mockInvoke.mockResolvedValue(undefined)
    await startDownload('whisper-tiny', { activate: true })
    expect(mockInvoke).toHaveBeenCalledWith('start_model_download', { id: 'whisper-tiny', activate: true })
    expect(mockInvoke).not.toHaveBeenCalledWith('set_model_override', expect.anything())
  })

  it('reports a refused download as not started', async () => {
    mockInvoke.mockRejectedValue({ code: 'unknown_model', message: 'That model isn’t available.' })
    expect(await startDownload('nope')).toBe(false)
  })
})

describe('active model', () => {
  it('names the catalog id of the model Rust reports', async () => {
    const catalog = [
      { id: 'whisper-tiny', displayName: 'Whisper Tiny' },
      { id: 'parakeet-v3', displayName: 'Parakeet V3' },
    ] as CatalogModel[]
    mockInvoke.mockImplementation(cmd => Promise.resolve(cmd === 'get_model_catalog'
      ? catalog
      : { modelName: 'Parakeet V3', downloaded: false }))

    const { result } = renderHook(() => useActiveModel(), { wrapper })

    await waitFor(() => expect(result.current.data).toEqual({ id: 'parakeet-v3', downloaded: false }))
  })
})
