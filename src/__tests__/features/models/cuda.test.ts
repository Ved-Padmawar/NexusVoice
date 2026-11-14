/**
 * The CUDA pack download. A failed start or download must reach the user
 * rather than vanish, and the CUDA dialog must open once, only where it
 * applies.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { createElement, type ReactNode } from 'react'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClientProvider } from '@tanstack/react-query'
import { useAppStore } from '../../../app/store/useAppStore'
import { queryClient } from '../../../app/queryClient'
import { downloadCuda, listenForCudaNotices, useCuda } from '../../../features/models/cuda'
import { queryKeys } from '../../../shared/queryKeys'
import type { CudaStatus } from '../../../shared/types'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }))

const mockInvoke = vi.mocked(invoke)
const mockListen = vi.mocked(listen)
const wrapper = ({ children }: { children: ReactNode }) => createElement(QueryClientProvider, { client: queryClient }, children)

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
    queryClient.clear()
    mockInvoke.mockResolvedValue({ ...absent, pack: 'installed' })
    const { unmount } = renderHook(() => useCuda(), { wrapper })
    await waitFor(() => expect(state().cudaOfferSeen).toBe(true))
    unmount()

    // Removed from About: the pack is absent again, but the offer stays retired.
    vi.useFakeTimers()
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
    const handlers = new Map<string, (e: { payload: unknown }) => void>()
    mockListen.mockImplementation(async (event, handler) => {
      handlers.set(event, handler as (e: { payload: unknown }) => void)
      return () => {}
    })
    queryClient.clear()
    const unverified: CudaStatus = { ...absent, pack: 'unverified' }
    mockInvoke.mockResolvedValue({ ...unverified, downloadProgress: 30 })
    renderHook(() => useCuda(), { wrapper })
    await waitFor(() => expect(cuda()?.downloadProgress).toBe(30))
    await listenForCudaNotices()

    // The error notice comes after Rust announced the download's end.
    mockInvoke.mockResolvedValue(unverified)
    vi.useFakeTimers()
    handlers.get('cuda-download-error')?.({ payload: 'offline' })
    await act(() => vi.runAllTimersAsync())

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
