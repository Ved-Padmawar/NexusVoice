import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { onlineManager, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { queryClient } from '../../lib/queries/client'
import { queryKeys } from '../../lib/queries/keys'
import { addTranscript, NO_FILTERS, PAGE_SIZE, useDeleteTranscript, useTranscripts, useTranscriptSearch, type TranscriptPage, type TranscriptPages } from '../../lib/queries/transcripts'
import { useUpdateDictionary } from '../../lib/queries/dictionary'
import type { Transcript } from '../../types'

vi.mock('sonner', () => ({ toast: { error: vi.fn() } }))
const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
const row = (id: number): Transcript => ({ id, content: `Transcript ${id}`, createdAt: `2026-09-02T12:00:${String(id % 60).padStart(2, '0')}Z`, wordCount: 2, durationSeconds: 1, targetApp: null })
const page = (rows: Transcript[]): TranscriptPage => {
  const last = rows.at(-1)
  return { rows, next: last && rows.length >= PAGE_SIZE ? { cursorCreatedAt: last.createdAt, cursorId: last.id } : undefined }
}
const pages = (rows: Transcript[]): TranscriptPages => ({ pages: [page(rows)], pageParams: [{ cursorCreatedAt: null, cursorId: null }] })
const ids = (data?: { pages: TranscriptPage[] }) => data?.pages.flatMap(p => p.rows).map(t => t.id)
const feedKey = queryKeys.transcripts(NO_FILTERS)
beforeEach(() => { queryClient.clear(); vi.mocked(invoke).mockReset().mockResolvedValue([]) })
afterEach(() => { cleanup(); queryClient.clear(); onlineManager.setOnline(true) })

describe('local Query cache', () => {
  it('preserves loaded pages and cursors when an existing older row is delivered again', async () => {
    const data = { pages: [page([row(3), row(2)]), page([row(1)])], pageParams: [{ cursorCreatedAt: null, cursorId: null }, { cursorCreatedAt: row(2).createdAt, cursorId: 2 }] }
    queryClient.setQueryData(feedKey, data)
    await addTranscript(row(1))
    expect(queryClient.getQueryData(feedKey)).toEqual(data)
  })

  it('keeps every loaded row, once, when a new transcript lands on a full first page', async () => {
    const firstPage = Array.from({ length: PAGE_SIZE }, (_, i) => row(PAGE_SIZE + 1 - i))
    const secondPage = [row(0)]
    queryClient.setQueryData(feedKey, {
      pages: [page(firstPage), page(secondPage)],
      pageParams: [{ cursorCreatedAt: null, cursorId: null }, { cursorCreatedAt: row(1).createdAt, cursorId: 1 }],
    })
    await addTranscript(row(999))
    const loaded = ids(queryClient.getQueryData<TranscriptPages>(feedKey))!
    expect(loaded).toEqual([999, ...firstPage.map(t => t.id), 0])
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
    await waitFor(() => expect(ids(result.current.data)).toEqual([2, 1]))
    await act(async () => { finishOld([row(1)]) })
    expect(ids(queryClient.getQueryData<TranscriptPages>(feedKey))).toEqual([2, 1])
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
    expect(ids(queryClient.getQueryData<TranscriptPages>(feedKey))).toEqual([2, 1])
  })

  it('invalidates cached searches and filtered feeds after a new transcript', async () => {
    const keys = [queryKeys.transcriptSearch('hello', NO_FILTERS), queryKeys.transcripts({ ...NO_FILTERS, sortAsc: true })]
    keys.forEach(key => queryClient.setQueryData(key, pages([])))
    await addTranscript(row(2))
    keys.forEach(key => expect(queryClient.getQueryState(key)?.isInvalidated).toBe(true))
    expect(invoke).not.toHaveBeenCalled()
  })

  it('removes a deleted transcript from every loaded page without refetching them', async () => {
    const searchKey = queryKeys.transcriptSearch('hello', NO_FILTERS)
    queryClient.setQueryData(feedKey, { pages: [page([row(3), row(2)]), page([row(1)])], pageParams: [{ cursorCreatedAt: null, cursorId: null }, { cursorCreatedAt: row(2).createdAt, cursorId: 2 }] })
    queryClient.setQueryData(searchKey, pages([row(2)]))
    const { result } = renderHook(() => useDeleteTranscript(), { wrapper })
    await act(async () => { await result.current.mutateAsync(2) })
    expect(queryClient.getQueryData<TranscriptPages>(feedKey)?.pages.map(p => p.rows.map(t => t.id))).toEqual([[3], [1]])
    expect(ids(queryClient.getQueryData<TranscriptPages>(searchKey))).toEqual([])
    expect(invoke).toHaveBeenCalledTimes(1)
    expect(vi.mocked(invoke).mock.calls[0][0]).toBe('delete_transcript')
  })

  it('still offers older pages after a delete shrinks the last loaded page', async () => {
    const rows = Array.from({ length: PAGE_SIZE }, (_, i) => row(PAGE_SIZE - i))
    queryClient.setQueryData(feedKey, pages(rows))
    const { result } = renderHook(() => ({ feed: useTranscripts(NO_FILTERS), del: useDeleteTranscript() }), { wrapper })
    await act(async () => { await result.current.del.mutateAsync(PAGE_SIZE) })
    await waitFor(() => expect(ids(result.current.feed.data)).toHaveLength(PAGE_SIZE - 1))
    expect(result.current.feed.hasNextPage).toBe(true)
  })

  it('keeps a deleted transcript out when a fetch already in flight returns it', async () => {
    queryClient.setQueryData(feedKey, pages([row(2), row(1)]))
    let finishStale!: (rows: Transcript[]) => void
    vi.mocked(invoke)
      .mockImplementationOnce(() => new Promise(r => { finishStale = r as typeof finishStale }))
      .mockImplementation((cmd) => Promise.resolve(cmd === 'delete_transcript' ? undefined : [row(2)]))
    const { result } = renderHook(() => ({ feed: useTranscripts(NO_FILTERS), del: useDeleteTranscript() }), { wrapper })
    act(() => { void result.current.feed.refetch() })
    await act(async () => { await result.current.del.mutateAsync(1) })
    await act(async () => { finishStale([row(2), row(1)]) })
    await waitFor(() => expect(ids(queryClient.getQueryData<TranscriptPages>(feedKey))).toEqual([2]))
    expect(result.current.feed.isFetching).toBe(false)
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
    await waitFor(() => expect(ids(result.current.data)).toEqual([2, 1]))
  })
})
