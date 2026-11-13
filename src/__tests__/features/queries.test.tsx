import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { focusManager, onlineManager, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { queryClient } from '../../app/queryClient'
import { queryKeys } from '../../shared/queryKeys'
import { addTranscript, fetchAllTranscripts, NO_FILTERS, PAGE_SIZE, patchRows, showRewritten, useDeleteTranscript, useTranscripts, useTranscriptSearch, type TranscriptPage, type TranscriptPages } from '../../features/history/queries'
import { refreshDictionaryUses, useDeleteDictionaryEntry, useDictionary, useUpdateDictionary } from '../../features/dictionary/queries'
import type { DictionaryResponse } from '../../shared/bindings'
import type { Transcript } from '../../shared/types'

type ToastOptions = { onAutoClose?: () => void; onDismiss?: () => void; action?: { onClick: () => void } }
const toasts = vi.hoisted(() => [] as ToastOptions[])
vi.mock('sonner', () => ({ toast: Object.assign((_: string, options: ToastOptions) => { toasts.push(options) }, { error: vi.fn(), dismiss: vi.fn() }) }))
/** The undo window ran out: the real delete goes ahead. */
const closeUndoToast = () => act(async () => { toasts.at(-1)?.onAutoClose?.() })
const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
const row = (id: number): Transcript => ({ id, content: `Transcript ${id}`, createdAt: `2026-09-02T12:00:${String(id % 60).padStart(2, '0')}Z`, wordCount: 2, durationSeconds: 1, targetApp: null, pinned: false, audioPath: null, sourceName: null, model: null, pending: false, revision: 0 })
const page = (rows: Transcript[]): TranscriptPage => {
  const last = rows.at(-1)
  return { rows, next: last && rows.length >= PAGE_SIZE ? { cursorCreatedAt: last.createdAt, cursorId: last.id } : undefined }
}
const pages = (rows: Transcript[]): TranscriptPages => ({ pages: [page(rows)], pageParams: [{ cursorCreatedAt: null, cursorId: null }] })
const ids = (data?: { pages: TranscriptPage[] }) => data?.pages.flatMap(p => p.rows).map(t => t.id)
const feedKey = queryKeys.transcripts(NO_FILTERS)
beforeEach(() => { queryClient.clear(); toasts.length = 0; vi.mocked(invoke).mockReset().mockResolvedValue([]) })
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

  it('patches a rewritten entry into the feed and reruns searches, whose matches its text decides', async () => {
    const searchKey = queryKeys.transcriptSearch('hello', NO_FILTERS)
    queryClient.setQueryData(feedKey, pages([row(1)]))
    queryClient.setQueryData(searchKey, pages([row(1)]))
    await showRewritten({ ...row(1), content: 'Rewritten' })
    expect(queryClient.getQueryData<TranscriptPages>(feedKey)?.pages[0].rows[0].content).toBe('Rewritten')
    expect(queryClient.getQueryState(feedKey)?.isInvalidated).toBe(false)
    expect(queryClient.getQueryState(searchKey)?.isInvalidated).toBe(true)
  })

  it('removes a deleted transcript from every loaded page without refetching them', async () => {
    const searchKey = queryKeys.transcriptSearch('hello', NO_FILTERS)
    queryClient.setQueryData(feedKey, { pages: [page([row(3), row(2)]), page([row(1)])], pageParams: [{ cursorCreatedAt: null, cursorId: null }, { cursorCreatedAt: row(2).createdAt, cursorId: 2 }] })
    queryClient.setQueryData(searchKey, pages([row(2)]))
    const { result } = renderHook(() => ({ feed: useTranscripts(NO_FILTERS), search: useTranscriptSearch('hello', NO_FILTERS), del: useDeleteTranscript() }), { wrapper })
    act(() => result.current.del(2))
    expect(result.current.feed.data?.pages.map(p => p.rows.map(t => t.id))).toEqual([[3], [1]])
    expect(ids(result.current.search.data)).toEqual([])
    await closeUndoToast()
    expect(invoke).toHaveBeenCalledTimes(1)
    expect(vi.mocked(invoke).mock.calls[0][0]).toBe('delete_transcript')
    expect(ids(queryClient.getQueryData<TranscriptPages>(searchKey))).toEqual([])
  })

  it('puts a deleted transcript back and never deletes it when Undo is clicked', async () => {
    queryClient.setQueryData(feedKey, pages([row(2), row(1)]))
    const { result } = renderHook(() => ({ feed: useTranscripts(NO_FILTERS), del: useDeleteTranscript() }), { wrapper })
    act(() => result.current.del(2))
    expect(ids(result.current.feed.data)).toEqual([1])
    // Sonner dismisses the toast after its action runs; that must not delete.
    act(() => { toasts.at(-1)?.action?.onClick(); toasts.at(-1)?.onDismiss?.() })
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === 'delete_transcript')).toBe(false)
    expect(ids(result.current.feed.data)).toEqual([2, 1])
  })

  it('puts the newest deleted transcript back on Ctrl+Z and never deletes it', async () => {
    queryClient.setQueryData(feedKey, pages([row(2), row(1)]))
    const { result } = renderHook(() => ({ feed: useTranscripts(NO_FILTERS), del: useDeleteTranscript() }), { wrapper })
    act(() => result.current.del(2))
    act(() => { window.dispatchEvent(new KeyboardEvent('keydown', { key: 'z', ctrlKey: true })) })
    act(() => toasts.at(-1)?.onDismiss?.())
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === 'delete_transcript')).toBe(false)
    expect(ids(result.current.feed.data)).toEqual([2, 1])
  })

  it('leaves Ctrl+Z to a focused text field and keeps the delete pending', async () => {
    queryClient.setQueryData(feedKey, pages([row(2), row(1)]))
    const { result } = renderHook(() => ({ feed: useTranscripts(NO_FILTERS), del: useDeleteTranscript() }), { wrapper })
    act(() => result.current.del(2))
    const input = document.body.appendChild(document.createElement('input'))
    act(() => { input.dispatchEvent(new KeyboardEvent('keydown', { key: 'z', ctrlKey: true, bubbles: true })) })
    input.remove()
    expect(ids(result.current.feed.data)).toEqual([1])
    await closeUndoToast()
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === 'delete_transcript')).toBe(true)
  })

  it('keeps a pending delete hidden when a refetch or export returns it, and gone once it commits; export skips an untranscribed import', async () => {
    queryClient.setQueryData(feedKey, pages([row(3), row(2), row(1)]))
    // The database still holds both rows until their undo windows close.
    vi.mocked(invoke).mockImplementation((cmd) => Promise.resolve(cmd === 'delete_transcript' ? undefined : [row(3), row(2), row(1)]))
    const { result } = renderHook(() => ({ feed: useTranscripts(NO_FILTERS), del: useDeleteTranscript() }), { wrapper })
    act(() => { result.current.del(2); result.current.del(1) })
    const [deleteTwo, deleteOne] = [...toasts]
    act(() => deleteOne.action?.onClick())
    await act(async () => { await result.current.feed.refetch() })
    expect(ids(result.current.feed.data)).toEqual([3, 1])
    let finishRead = () => {}
    vi.mocked(invoke).mockImplementation(() => new Promise(resolve => { finishRead = () => resolve([{ ...row(4), pending: true }, row(3), row(2), row(1)]) }))
    const exported = fetchAllTranscripts('', NO_FILTERS)
    // Row 2's undo window closes while the export is still reading it.
    vi.mocked(invoke).mockImplementation((cmd) => Promise.resolve(cmd === 'delete_transcript' ? undefined : [row(3), row(1)]))
    await act(async () => { deleteTwo.onAutoClose?.() })
    await waitFor(() => expect(ids(result.current.feed.data)).toEqual([3, 1]))
    finishRead()
    expect((await exported).map(t => t.id)).toEqual([3, 1])
    expect(ids(queryClient.getQueryData<TranscriptPages>(feedKey))).toEqual([3, 1])
  })

  it('keeps a pending dictionary delete hidden when the list refetches', async () => {
    const entry = (id: number): DictionaryResponse => ({ id, term: `term ${id}`, replacement: `fix ${id}`, hits: 0, createdAt: '2026-09-02T12:00:00Z' })
    vi.mocked(invoke).mockResolvedValue([entry(1), entry(2)])
    const { result } = renderHook(() => ({ list: useDictionary(), del: useDeleteDictionaryEntry() }), { wrapper })
    await waitFor(() => expect(result.current.list.data).toHaveLength(2))
    act(() => result.current.del(entry(2)))
    await act(async () => { await refreshDictionaryUses() })
    expect(result.current.list.data?.map(e => e.id)).toEqual([1])
  })

  it('still offers older pages after a delete shrinks the last loaded page', async () => {
    const rows = Array.from({ length: PAGE_SIZE }, (_, i) => row(PAGE_SIZE - i))
    queryClient.setQueryData(feedKey, pages(rows))
    const { result } = renderHook(() => ({ feed: useTranscripts(NO_FILTERS), del: useDeleteTranscript() }), { wrapper })
    act(() => result.current.del(PAGE_SIZE))
    await waitFor(() => expect(ids(result.current.feed.data)).toHaveLength(PAGE_SIZE - 1))
    expect(result.current.feed.hasNextPage).toBe(true)
  })

  it('keeps a deleted transcript out, and a changed one changed, when a fetch already in flight returns the old rows', async () => {
    queryClient.setQueryData(feedKey, pages([row(2), row(1)]))
    let finishStale!: (rows: Transcript[]) => void
    vi.mocked(invoke)
      .mockImplementationOnce(() => new Promise(r => { finishStale = r as typeof finishStale }))
      .mockImplementation((cmd) => Promise.resolve(cmd === 'delete_transcript' ? undefined : [row(2)]))
    const { result } = renderHook(() => ({ feed: useTranscripts(NO_FILTERS), del: useDeleteTranscript() }), { wrapper })
    act(() => { void result.current.feed.refetch() })
    act(() => result.current.del(1))
    await closeUndoToast()
    await act(async () => { finishStale([row(2), row(1)]) })
    await waitFor(() => expect(ids(queryClient.getQueryData<TranscriptPages>(feedKey))).toEqual([2]))
    await waitFor(() => expect(queryClient.getQueryState(feedKey)?.fetchStatus).toBe('idle'))

    // Changes in place too, like audio deleted under a read that still has it:
    // all in one write, which restarts that read once.
    const withAudio = (id: number) => ({ ...row(id), audioPath: `/${id}.wav` })
    queryClient.setQueryData(feedKey, pages([withAudio(3), withAudio(2)]))
    vi.mocked(invoke)
      .mockReset()
      .mockImplementationOnce(() => new Promise(r => { finishStale = r as typeof finishStale }))
      .mockImplementation(() => Promise.resolve([row(3), row(2)]))
    act(() => { void result.current.feed.refetch() })
    const lost = (entry: Transcript) => ({ ...entry, audioPath: null })
    await act(async () => { await patchRows(new Map([[3, lost], [2, lost]])) })
    await act(async () => { finishStale([withAudio(3), withAudio(2)]) })
    await waitFor(() => expect(queryClient.getQueryState(feedKey)?.fetchStatus).toBe('idle'))
    expect(queryClient.getQueryData<TranscriptPages>(feedKey)?.pages[0].rows.map(r => r.audioPath)).toEqual([null, null])
    expect(invoke).toHaveBeenCalledTimes(2)
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

describe('coordination contracts', () => {
  it('settles a dictionary save only once the list shows it', async () => {
    let answerList: (entries: unknown[]) => void = () => {}
    let listCalls = 0
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === 'get_dictionary') {
        listCalls += 1
        return listCalls === 1 ? Promise.resolve([]) : new Promise(resolve => { answerList = resolve })
      }
      return Promise.resolve(null)
    })
    const { result } = renderHook(() => ({ list: useDictionary(), update: useUpdateDictionary() }), { wrapper })
    await waitFor(() => expect(result.current.list.isSuccess).toBe(true))
    let settled = false
    act(() => { void result.current.update.mutateAsync({ term: 'teh', replacement: 'the' }).then(() => { settled = true }) })
    await waitFor(() => expect(listCalls).toBe(2))
    expect(settled).toBe(false)
    await act(async () => { answerList([{ id: 1, term: 'teh', replacement: 'the', hits: 0 }]) })
    await waitFor(() => expect(settled).toBe(true))
    expect(result.current.list.data).toHaveLength(1)
  })

  it('refreshes a search whose refetch was skipped while the window was hidden', async () => {
    const searches = () => vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === 'search_transcripts').length
    renderHook(() => useTranscriptSearch('foo', NO_FILTERS), { wrapper })
    await waitFor(() => expect(searches()).toBe(1))
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true)
    await act(async () => { await addTranscript(row(7)) })
    expect(searches()).toBe(1)
    hidden.mockReturnValue(false)
    act(() => { focusManager.setFocused(true) })
    await waitFor(() => expect(searches()).toBe(2))
    focusManager.setFocused(undefined)
  })

  it('re-reads dictionary uses after a dictation, or once the hidden window is shown', async () => {
    const reads = () => vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === 'get_dictionary').length
    renderHook(() => useDictionary(), { wrapper })
    await waitFor(() => expect(reads()).toBe(1))
    await act(async () => { await refreshDictionaryUses() })
    await waitFor(() => expect(reads()).toBe(2))
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true)
    await act(async () => { await refreshDictionaryUses() })
    expect(reads()).toBe(2)
    hidden.mockReturnValue(false)
    act(() => { focusManager.setFocused(true) })
    await waitFor(() => expect(reads()).toBe(3))
    focusManager.setFocused(undefined)
  })
})
