/**
 * The CUDA pack download. A start that fails must not leave the bar stuck at
 * 0%, a download error must reach the user rather than vanish, and the CUDA
 * dialog must open once, only where it applies.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useAppStore } from '../../../app/store/useAppStore'
import { queryClient } from '../../../app/queryClient'
import { downloadCuda, listenForCudaEvents, refreshCuda } from '../../../features/models/cuda'
import { queryKeys } from '../../../shared/queryKeys'
import type { CudaStatus } from '../../../shared/types'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }))
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }))

const mockInvoke = vi.mocked(invoke)
const mockListen = vi.mocked(listen)

const absent: CudaStatus = { supported: true, pack: 'absent', downloadProgress: null, verifying: false }
const state = () => useAppStore.getState()
const cuda = () => queryClient.getQueryData<CudaStatus>(queryKeys.cuda)
const setCuda = (status: CudaStatus) => queryClient.setQueryData(queryKeys.cuda, status)

beforeEach(() => {
  vi.clearAllMocks()
  queryClient.clear()
  setCuda(absent)
  useAppStore.setState({ cudaError: null, cudaOfferOpen: false, cudaOfferSeen: false, cudaRetryShown: false, modelChosen: true })
})

afterEach(() => {
  vi.useRealTimers()
})

describe('cuda pack', () => {
  it('surfaces a failed start and restores the real status', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'start_cuda_download') throw { code: 'unsupported', message: 'CUDA needs an NVIDIA GPU' }
      return absent
    })

    await downloadCuda()

    expect(state().cudaError).toBe('CUDA needs an NVIDIA GPU')
    expect(cuda()?.downloadProgress).toBeNull()
  })

  it('listens before reading status, tracks progress events and reports a download error', async () => {
    const handlers = new Map<string, (e: { payload: unknown }) => void>()
    mockListen.mockImplementation(async (event, handler) => {
      handlers.set(event, handler as (e: { payload: unknown }) => void)
      return () => {}
    })
    let listenersAtRead = 0
    mockInvoke.mockImplementation(async () => {
      listenersAtRead = handlers.size
      return absent
    })
    await listenForCudaEvents()
    expect(listenersAtRead).toBe(handlers.size)

    handlers.get('cuda-download-progress')?.({ payload: 42 })
    expect(cuda()?.downloadProgress).toBe(42)

    handlers.get('cuda-download-verifying')?.({ payload: null })
    expect(cuda()?.verifying).toBe(true)

    handlers.get('cuda-download-error')?.({ payload: 'checksum mismatch' })
    expect(state().cudaError).toBe('checksum mismatch')
  })

  it('offers CUDA once, after a delay, and never again once shown', () => {
    vi.useFakeTimers()
    state().offerCuda()
    expect(state().cudaOfferOpen).toBe(false)

    vi.runAllTimers()
    expect(state().cudaOfferOpen).toBe(true)

    state().closeCudaOffer()
    state().offerCuda()
    vi.runAllTimers()
    expect(state().cudaOfferOpen).toBe(false)
  })

  it('never offers CUDA again once it was installed, even after removal', async () => {
    vi.useFakeTimers()
    mockInvoke.mockResolvedValueOnce({ ...absent, pack: 'installed' })
    await refreshCuda()

    // Removed from About: the pack is absent again, but the offer stays retired.
    setCuda(absent)
    state().offerCuda()
    vi.runAllTimers()
    expect(state().cudaOfferOpen).toBe(false)
  })

  it('does not offer CUDA before a model is chosen or where it cannot run', () => {
    vi.useFakeTimers()
    useAppStore.setState({ modelChosen: false })
    state().offerCuda()
    vi.runAllTimers()
    expect(state().cudaOfferOpen).toBe(false)

    useAppStore.setState({ modelChosen: true })
    setCuda({ ...absent, supported: false })
    state().offerCuda()
    vi.runAllTimers()
    expect(state().cudaOfferOpen).toBe(false)
  })

  it('asks once a session to retry an unverified pack, even after the offer was seen', () => {
    vi.useFakeTimers()
    useAppStore.setState({ cudaOfferSeen: true })
    setCuda({ ...absent, pack: 'unverified' })
    state().offerCuda()
    vi.runAllTimers()
    expect(state().cudaOfferOpen).toBe(true)

    state().closeCudaOffer()
    state().offerCuda()
    vi.runAllTimers()
    expect(state().cudaOfferOpen).toBe(false)
  })

  it('asks for a retry when replacing an unverified pack fails', async () => {
    vi.useFakeTimers()
    const handlers = new Map<string, (e: { payload: unknown }) => void>()
    mockListen.mockImplementation(async (event, handler) => {
      handlers.set(event, handler as (e: { payload: unknown }) => void)
      return () => {}
    })
    const unverified: CudaStatus = { ...absent, pack: 'unverified' }
    mockInvoke.mockResolvedValue({ ...unverified, downloadProgress: 30 })
    await listenForCudaEvents()
    expect(cuda()?.downloadProgress).toBe(30)

    mockInvoke.mockResolvedValue(unverified)
    handlers.get('cuda-download-error')?.({ payload: 'offline' })
    await vi.runAllTimersAsync()

    expect(state().cudaOfferOpen).toBe(true)
    expect(state().cudaError).toBeNull()
  })

  it('does not offer CUDA while it is already downloading', () => {
    // Started from About before the timer fired: the offer would be redundant.
    vi.useFakeTimers()
    state().offerCuda()
    setCuda({ ...absent, downloadProgress: 10 })
    vi.runAllTimers()
    expect(state().cudaOfferOpen).toBe(false)
  })
})
