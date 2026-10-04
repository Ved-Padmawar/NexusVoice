/**
 * The CUDA pack download. A start that fails must not leave the bar stuck at
 * 0%, and a download error must reach the user rather than vanish.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useAppStore } from '../../store/useAppStore'
import { COMMANDS } from '../../lib/commands'
import { EVENTS } from '../../lib/events'
import type { CudaStatus } from '../../types'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }))
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }))

const mockInvoke = vi.mocked(invoke)
const mockListen = vi.mocked(listen)

const absent: CudaStatus = { supported: true, pack: 'absent', downloadProgress: null }
const state = () => useAppStore.getState()

beforeEach(() => {
  vi.clearAllMocks()
  useAppStore.setState({ cuda: absent, cudaError: null })
})

describe('cuda slice', () => {
  it('surfaces a failed start and restores the real status', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === COMMANDS.START_CUDA_DOWNLOAD) throw { code: 'unsupported', message: 'CUDA needs an NVIDIA GPU' }
      return absent
    })

    await state().downloadCuda()

    expect(state().cudaError).toBe('CUDA needs an NVIDIA GPU')
    expect(state().cuda?.downloadProgress).toBeNull()
  })

  it('tracks progress events and reports a download error', async () => {
    const handlers = new Map<string, (e: { payload: unknown }) => void>()
    mockListen.mockImplementation(async (event, handler) => {
      handlers.set(event, handler as (e: { payload: unknown }) => void)
      return () => {}
    })
    mockInvoke.mockResolvedValue(absent)
    await state().listenForCudaEvents()

    handlers.get(EVENTS.CUDA_DOWNLOAD_PROGRESS)?.({ payload: 42 })
    expect(state().cuda?.downloadProgress).toBe(42)

    handlers.get(EVENTS.CUDA_DOWNLOAD_ERROR)?.({ payload: 'checksum mismatch' })
    expect(state().cudaError).toBe('checksum mismatch')
  })
})
