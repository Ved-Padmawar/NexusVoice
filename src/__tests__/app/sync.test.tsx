import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen, type EventCallback } from '@tauri-apps/api/event'
import { queryClient } from '../../app/queryClient'
import { caughtUp, followRust } from '../../app/sync'
import { useLiveChoice } from '../../app/useLiveChoice'
import { queryKeys } from '../../shared/queryKeys'
import { NO_FILTERS, prefetchStats, useDailyWords, useStats, useTranscriptSearch } from '../../features/history/queries'
import type { Topic, UsageStatsResponse } from '../../shared/bindings'
import type { Transcript } from '../../shared/types'

const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
const stats = (totalWords: number): UsageStatsResponse => ({ totalWords, speakingTimeSeconds: 0, totalSessions: 1, avgPaceWpm: 0 })
const row = (id: number): Transcript => ({ id, content: 'hello', createdAt: '2026-09-02 12:00:00', wordCount: 1, durationSeconds: 1, targetApp: null, pinned: false, audioPath: null, sourceName: null, model: null, pending: false, revision: 0 })

/** Every read Rust answers by hand, in the order the app asked. */
let reads: { resolve: (value: unknown) => void; reject: (error: unknown) => void }[] = []
let announce: (topic: Topic) => void = () => {}

beforeEach(async () => {
  queryClient.clear()
  reads = []
  vi.mocked(invoke).mockReset().mockImplementation(() => new Promise((resolve, reject) => { reads.push({ resolve, reject }) }))
  vi.mocked(listen).mockImplementation(async (event, handler) => {
    if (event === 'changed') announce = topic => (handler as EventCallback<Topic>)({ event, id: 0, payload: topic })
    return () => {}
  })
  await followRust()
})
afterEach(() => { cleanup(); queryClient.clear(); vi.restoreAllMocks() })

/** Answer read `n` (0-based), letting the cache take it in. */
const answer = (n: number, value: unknown) => act(async () => { reads[n].resolve(value) })
const fail = (n: number) => act(async () => { reads[n].reject({ code: 'db', message: 'locked' }) })

describe('following Rust', () => {
  it('reads a query again when an owner it reads announces a change, and only then', async () => {
    const { result } = renderHook(() => useStats(), { wrapper })
    await answer(0, stats(1))
    act(() => announce('dictionary'))
    expect(reads).toHaveLength(1)
    act(() => announce('history'))
    await answer(1, stats(2))
    await waitFor(() => expect(result.current.data?.totalWords).toBe(2))
  })

  it('shows each read as it lands during a burst of changes, then the newest', async () => {
    const { result } = renderHook(() => useStats(), { wrapper })
    await answer(0, stats(1))
    act(() => announce('history'))
    // Read 1 is in flight: these let it land instead of cancelling it.
    act(() => announce('history'))
    act(() => announce('history'))
    expect(reads).toHaveLength(2)
    await answer(1, stats(2))
    await waitFor(() => expect(result.current.data?.totalWords).toBe(2))
    await waitFor(() => expect(reads).toHaveLength(3))
    await answer(2, stats(4))
    await waitFor(() => expect(result.current.data?.totalWords).toBe(4))
  })

  it('shows a first read that a burst of changes lands on, then reads once more', async () => {
    const { result } = renderHook(() => useTranscriptSearch('hello', NO_FILTERS), { wrapper })
    await waitFor(() => expect(reads).toHaveLength(1))
    act(() => { announce('history'); announce('history'); announce('history') })
    expect(reads).toHaveLength(1)
    await answer(0, [row(1)])
    await waitFor(() => expect(result.current.data?.pages[0].rows.map(r => r.id)).toEqual([1]))
    await waitFor(() => expect(reads).toHaveLength(2))
    await answer(1, [row(2), row(1)])
    await waitFor(() => expect(result.current.data?.pages[0].rows.map(r => r.id)).toEqual([2, 1]))
  })

  it('reads a closed screen again when it opens after a change, even one that landed mid-read', async () => {
    const prefetched = prefetchStats()
    act(() => announce('history'))
    await answer(0, stats(1))
    await prefetched
    const { result } = renderHook(() => useStats(), { wrapper })
    await waitFor(() => expect(reads).toHaveLength(2))
    await answer(1, stats(2))
    await waitFor(() => expect(result.current.data?.totalWords).toBe(2))
  })

  it('tries a failed read again, so the change it was for still shows', async () => {
    const { result } = renderHook(() => useStats(), { wrapper })
    await answer(0, stats(1))
    act(() => announce('history'))
    await fail(1)
    await waitFor(() => expect(reads).toHaveLength(3))
    await answer(2, stats(2))
    await waitFor(() => expect(result.current.data?.totalWords).toBe(2))
  })

  it('leaves a hidden window stale until it is shown, then reads it', async () => {
    const { result } = renderHook(() => useStats(), { wrapper })
    await answer(0, stats(1))
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true)
    act(() => announce('history'))
    expect(reads).toHaveLength(1)
    hidden.mockReturnValue(false)
    act(() => { window.dispatchEvent(new Event('visibilitychange')) })
    await waitFor(() => expect(reads).toHaveLength(2))
    await answer(1, stats(2))
    await waitFor(() => expect(result.current.data?.totalWords).toBe(2))
  })
})

describe('caughtUp', () => {
  it('waits past a first read that began before the save', async () => {
    renderHook(() => useStats(), { wrapper })
    await waitFor(() => expect(reads).toHaveLength(1))
    let done = false
    act(() => { void caughtUp('history').then(() => { done = true }) })
    await waitFor(() => expect(reads).toHaveLength(2))
    await answer(0, stats(1))
    expect(done).toBe(false)
    await answer(1, stats(2))
    await waitFor(() => expect(done).toBe(true))
  })

  it('waits for every query on screen, not the first to land', async () => {
    renderHook(() => ({ stats: useStats(), daily: useDailyWords(7) }), { wrapper })
    await waitFor(() => expect(reads).toHaveLength(2))
    await answer(0, stats(1))
    await answer(1, [])
    let done = false
    act(() => { void caughtUp('history').then(() => { done = true }) })
    await waitFor(() => expect(reads).toHaveLength(4))
    await answer(2, stats(2))
    expect(done).toBe(false)
    await answer(3, [])
    await waitFor(() => expect(done).toBe(true))
  })
})

describe('useLiveChoice', () => {
  /** Saves Rust answers by hand, in the order they were made. */
  const writes: { value: string; resolve: () => void; reject: () => void }[] = []
  const save = (value: string) => new Promise<void>((resolve, reject) => {
    writes.push({ value, resolve, reject: () => reject({ code: 'io', message: 'disk full' }) })
  })
  // A choice outlives its component, so each test picks a part of its own.
  const renderChoice = (part: string) => renderHook(() => {
    useStats()
    return useLiveChoice(queryKeys.stats, save, () => {}, part)
  }, { wrapper })
  beforeEach(() => { writes.length = 0 })

  /** Read `n` and both its retries fail. */
  const failReadBack = async (n: number) => {
    for (const read of [n, n + 1, n + 2]) {
      await waitFor(() => expect(reads).toHaveLength(read + 1))
      await fail(read)
    }
  }

  it('saves one pick at a time, the newest next', async () => {
    const { result } = renderChoice('one-at-a-time')
    await answer(0, stats(1))
    act(() => { result.current.choose('a'); result.current.choose('b'); result.current.choose('c') })
    expect(writes.map(w => w.value)).toEqual(['a'])
    await act(async () => { writes[0].resolve() })
    expect(writes.map(w => w.value)).toEqual(['a', 'c'])
  })

  it('reads back after a first read that began before the save', async () => {
    const { result } = renderChoice('first-read')
    await waitFor(() => expect(reads).toHaveLength(1))
    act(() => { result.current.choose('saved') })
    await act(async () => { writes[0].resolve() })
    await waitFor(() => expect(reads).toHaveLength(2))
    await answer(0, stats(1))
    expect(result.current.pending?.value).toBe('saved')
    await answer(1, stats(2))
    await waitFor(() => expect(result.current.pending).toBeNull())
  })

  it('keeps a saved pick through reads that keep failing, until a later one lands', async () => {
    const { result } = renderChoice('failing-reads')
    await answer(0, stats(1))
    act(() => { result.current.choose('saved') })
    await act(async () => { writes[0].resolve() })
    // A change lands while the read back is in flight; its reread waits for it.
    await waitFor(() => expect(reads).toHaveLength(2))
    act(() => announce('history'))
    await failReadBack(1)
    expect(result.current.pending?.value).toBe('saved')
    await answer(4, stats(2))
    await waitFor(() => expect(result.current.pending).toBeNull())
  })

  it('shows the save that landed when a later pick is refused', async () => {
    const { result } = renderChoice('refused')
    await answer(0, stats(1))
    act(() => { result.current.choose('saved') })
    await act(async () => { writes[0].resolve() })
    await failReadBack(1)
    act(() => { result.current.choose('refused') })
    await act(async () => { writes[1].reject() })
    expect(result.current.pending?.value).toBe('saved')
  })

  it('keeps one queue for a value when its screen closes and reopens mid-save', async () => {
    const first = renderChoice('reopened')
    await answer(0, stats(1))
    act(() => { first.result.current.choose('older'); first.result.current.choose('queued') })
    first.unmount()
    const { result } = renderChoice('reopened')
    expect(result.current.pending?.value).toBe('queued')
    act(() => { result.current.choose('newest') })
    await act(async () => { writes[0].resolve() })
    expect(writes.map(w => w.value)).toEqual(['older', 'newest'])
  })

  it('keeps a pick shown while a pick of another part of its query reads back', async () => {
    const theme = renderChoice('theme')
    const style = renderChoice('style')
    await answer(0, stats(1))
    act(() => { theme.result.current.choose('aurora') })
    await act(async () => { writes[0].resolve() })
    await waitFor(() => expect(reads).toHaveLength(2))
    act(() => { style.result.current.choose('memo') })
    await act(async () => { writes[1].resolve() })
    await waitFor(() => expect(reads).toHaveLength(3))
    expect(theme.result.current.pending?.value).toBe('aurora')
    await answer(2, stats(2))
    await waitFor(() => expect(theme.result.current.pending).toBeNull())
  })
})
