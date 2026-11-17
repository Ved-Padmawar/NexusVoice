/**
 * The CUDA pack download. A failed start or download must reach the user
 * rather than vanish, and the CUDA dialog must open once, only where it
 * applies.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { act, renderHook } from '@testing-library/react'
import { useAppStore } from '../../../app/store/useAppStore'
import { downloadCuda, listenForCudaNotices, useCuda } from '../../../features/models/cuda'
import type { CudaStatus } from '../../../shared/types'
import { mirror, settingsAt, type Send } from '../../utils'

const mockInvoke = vi.mocked(invoke)
const mockListen = vi.mocked(listen)

const absent: CudaStatus = { supported: true, pack: 'absent', downloadProgress: null, verifying: false, attempt: 0 }
const state = () => useAppStore.getState()

/** Rust's CUDA status, sent to this window. */
let rev = 0
let send: Send
const setCuda = (status: CudaStatus) => send({ slice: 'cuda', rev: ++rev, value: status })

beforeEach(async () => {
  vi.clearAllMocks()
  rev = 0
  send = await mirror([settingsAt(0), { slice: 'cuda', rev: 0, value: absent }])
  useAppStore.setState({ cudaError: null, cudaOfferOpen: false, cudaOfferSeen: false, cudaRetryShown: false, modelChosen: true })
})

afterEach(() => {
  vi.useRealTimers()
})

describe('cuda pack', () => {
  it('says why Rust refused to start the download', async () => {
    mockInvoke.mockRejectedValue({ code: 'unsupported', message: 'CUDA needs an NVIDIA GPU' })

    await downloadCuda()

    expect(state().cudaError).toBe('CUDA needs an NVIDIA GPU')
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
    await setCuda({ ...absent, pack: 'installed' })
    const { unmount } = renderHook(() => useCuda())
    expect(state().cudaOfferSeen).toBe(true)
    unmount()

    // Removed from About: the pack is absent again, but the offer stays retired.
    vi.useFakeTimers()
    await setCuda(absent)
    state().offerCuda()
    vi.runAllTimers()
    expect(state().cudaOfferOpen).toBe(false)
  })

  it('does not offer CUDA before a model is chosen or where it cannot run', async () => {
    vi.useFakeTimers()
    useAppStore.setState({ modelChosen: false })
    state().offerCuda()
    vi.runAllTimers()
    expect(state().cudaOfferOpen).toBe(false)

    useAppStore.setState({ modelChosen: true })
    await setCuda({ ...absent, supported: false })
    state().offerCuda()
    vi.runAllTimers()
    expect(state().cudaOfferOpen).toBe(false)
  })

  it('asks once a session to retry an unverified pack, even after the offer was seen', async () => {
    vi.useFakeTimers()
    useAppStore.setState({ cudaOfferSeen: true })
    await setCuda({ ...absent, pack: 'unverified' })
    state().offerCuda()
    vi.runAllTimers()
    expect(state().cudaOfferOpen).toBe(true)

    state().closeCudaOffer()
    state().offerCuda()
    vi.runAllTimers()
    expect(state().cudaOfferOpen).toBe(false)
  })

  it('asks for a retry when replacing an unverified pack fails', async () => {
    const handlers = new Map<string, (e: { payload: unknown }) => void>()
    mockListen.mockImplementation(async (event, handler) => {
      handlers.set(event, handler as (e: { payload: unknown }) => void)
      return () => {}
    })
    const unverified: CudaStatus = { ...absent, pack: 'unverified' }
    await setCuda({ ...unverified, downloadProgress: 30 })
    await listenForCudaNotices()

    // The error notice names the status revision that shows the download ended.
    vi.useFakeTimers()
    handlers.get('cuda-download-error')?.({ payload: { attempt: 0, message: 'offline', after: { rev: rev + 1 } } })
    await setCuda(unverified)
    await act(() => vi.runAllTimersAsync())

    expect(state().cudaOfferOpen).toBe(true)
    expect(state().cudaError).toBeNull()
  })

  it('does not offer CUDA while it is already downloading', async () => {
    // Started from About before the timer fired: the offer would be redundant.
    vi.useFakeTimers()
    state().offerCuda()
    await setCuda({ ...absent, downloadProgress: 10 })
    vi.runAllTimers()
    expect(state().cudaOfferOpen).toBe(false)
  })
})
