import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { onlineManager, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { queryClient } from '../../lib/queries/client'
import { queryKeys } from '../../lib/queries/keys'
import { addTranscript, NO_FILTERS, PAGE_SIZE, useTranscripts, useTranscriptSearch, type TranscriptPages } from '../../lib/queries/transcripts'
import { useUpdateDictionary } from '../../lib/queries/dictionary'
import type { Transcript } from '../../types'

vi.mock('sonner', () => ({ toast: { error: vi.fn() } }))
const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
const row = (id: number): Transcript => ({ id, content: `Transcript ${id}`, createdAt: `2026-09-02T12:00:${String(id % 60).padStart(2, '0')}Z`, wordCount: 2, durationSeconds: 1, targetApp: null })
const pages = (rows: Transcript[]): TranscriptPages => ({ pages: [rows], pageParams: [{ cursorCreatedAt: null, cursorId: null }] })
const feedKey = queryKeys.transcripts(NO_FILTERS)
beforeEach(() => { queryClient.clear(); vi.mocked(invoke).mockReset().mockResolvedValue([]) })
afterEach(() => { cleanup(); queryClient.clear(); onlineManager.setOnline(true) })

describe('local Query cache', () => {
  it('preserves loaded pages and cursors when an existing older row is delivered again', async () => {
    const data = { pages: [[row(3), row(2)], [row(1)]], pageParams: [{ cursorCreatedAt: null, cursorId: null }, { cursorCreatedAt: row(2).createdAt, cursorId: 2 }] }
    queryClient.setQueryData(feedKey, data)
    await addTranscript(row(1))
    expect(queryClient.getQueryData(feedKey)).toEqual(data)
  })

  it('holds the first page at PAGE_SIZE so a new row never duplicates the next page', async () => {
    const firstPage = Array.from({ length: PAGE_SIZE }, (_, i) => row(PAGE_SIZE + 1 - i))
    const secondPage = [row(0)]
    queryClient.setQueryData(feedKey, {
      pages: [firstPage, secondPage],
      pageParams: [{ cursorCreatedAt: null, cursorId: null }, { cursorCreatedAt: row(1).createdAt, cursorId: 1 }],
    })
    await addTranscript(row(999))
    const loaded = queryClient.getQueryData<TranscriptPages>(feedKey)!
    const ids = loaded.pages.flat().map(t => t.id)
    expect(loaded.pages[0]).toHaveLength(PAGE_SIZE)
    expect(new Set(ids).size).toBe(ids.length)
  })

  it('fetches only the search when the underlying feed is disabled', async () => {
    const { result } = renderHook(() => {
      useTranscripts(NO_FILTERS, false)
      return { ...useTranscriptSearch('hello', NO_FILTERS) }
    }, { wrapper })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(invoke).toHaveBeenCalledTimes(1)
    expect(vi.mocked(invoke).mock.calls[0][0]).toBe('search_transcripts')
  })

  it('restarts an initial fetch when a new event arrives before any cached data', async () => {
    let finishOld!: (rows: Transcript[]) => void
    vi.mocked(invoke)
      .mockImplementationOnce(() => new Promise(r => { finishOld = r as typeof finishOld }))
      .mockResolvedValue([row(2), row(1)])
    const { result } = renderHook(() => ({ ...useTranscripts(NO_FILTERS) }), { wrapper })
    await act(async () => { await addTranscript(row(2)) })
    await waitFor(() => expect(result.current.data?.pages.flat().map(t => t.id)).toEqual([2, 1]))
    await act(async () => { finishOld([row(1)]) })
    expect(queryClient.getQueryData<TranscriptPages>(feedKey)?.pages.flat().map(t => t.id)).toEqual([2, 1])
  })

  it('loads local transcripts while the network is offline', async () => {
    onlineManager.setOnline(false)
    const { result } = renderHook(() => ({ ...useTranscripts(NO_FILTERS) }), { wrapper })
    await waitFor(() => expect(result.current.isSuccess).toBe(true), { timeout: 200 })
    expect(invoke).toHaveBeenCalledTimes(1)
  })

  it('writes the local dictionary while the network is offline', async () => {
    onlineManager.setOnline(false)
    const { result } = renderHook(() => useUpdateDictionary(), { wrapper })
    act(() => result.current.mutate({ term: 'teh', replacement: 'the' }))
    await waitFor(() => expect(result.current.isSuccess).toBe(true), { timeout: 200 })
    expect(invoke).toHaveBeenCalledTimes(1)
  })

  it('keeps older pages reachable after a new transcript fills an already full page', async () => {
    const rows = Array.from({ length: PAGE_SIZE }, (_, i) => row(PAGE_SIZE - i))
    queryClient.setQueryData(feedKey, pages(rows))
    const { result } = renderHook(() => ({ ...useTranscripts(NO_FILTERS) }), { wrapper })
    await act(async () => { await addTranscript(row(100)) })
    expect(result.current.hasNextPage).toBe(true)
    await act(async () => { await result.current.fetchNextPage() })
    expect(invoke).toHaveBeenCalledWith('get_transcripts', { page: { ...NO_FILTERS, limit: PAGE_SIZE, cursorCreatedAt: rows.at(-1)!.createdAt, cursorId: 1 } })
  })

  it('does not duplicate a transcript when an event is delivered twice', async () => {
    queryClient.setQueryData(feedKey, pages([row(1)]))
    await addTranscript(row(2))
    await addTranscript(row(2))
    expect(queryClient.getQueryData<TranscriptPages>(feedKey)?.pages.flat().map(t => t.id)).toEqual([2, 1])
  })

  it('invalidates cached searches and filtered feeds after a new transcript', async () => {
    const keys = [queryKeys.transcriptSearch('hello', NO_FILTERS), queryKeys.transcripts({ ...NO_FILTERS, sortAsc: true })]
    keys.forEach(key => queryClient.setQueryData(key, pages([])))
    await addTranscript(row(2))
    keys.forEach(key => expect(queryClient.getQueryState(key)?.isInvalidated).toBe(true))
    expect(invoke).not.toHaveBeenCalled()
  })

  it('prevents an older in-flight response from erasing a new transcript', async () => {
    queryClient.setQueryData(feedKey, pages([row(1)]))
    let resolve!: (rows: Transcript[]) => void
    vi.mocked(invoke).mockImplementation(() => new Promise(r => { resolve = r as typeof resolve }))
    const { result } = renderHook(() => ({ ...useTranscripts(NO_FILTERS) }), { wrapper })
    let request!: ReturnType<typeof result.current.refetch>
    act(() => { request = result.current.refetch() })
    await act(async () => { await addTranscript(row(2)) })
    await act(async () => { resolve([row(1)]); await request })
    await waitFor(() => expect(result.current.data?.pages.flat().map(t => t.id)).toEqual([2, 1]))
  })
})
