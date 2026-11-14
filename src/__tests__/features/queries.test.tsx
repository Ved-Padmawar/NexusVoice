import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { onlineManager, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { queryClient } from '../../app/queryClient'
import { queryKeys } from '../../shared/queryKeys'
import { fetchAllTranscripts, NO_FILTERS, useDeleteTranscript, useTranscripts, useTranscriptSearch } from '../../features/history/queries'
import { useDeleteDictionaryEntry, useDictionary, useUpdateDictionary } from '../../features/dictionary/queries'
import type { DictionaryResponse } from '../../shared/bindings'
import type { Transcript } from '../../shared/types'

type ToastOptions = { onAutoClose?: () => void; onDismiss?: () => void; action?: { onClick: () => void } }
const toasts = vi.hoisted(() => [] as ToastOptions[])
vi.mock('sonner', () => ({ toast: Object.assign((_: string, options: ToastOptions) => { toasts.push(options) }, { error: vi.fn(), dismiss: vi.fn() }) }))
/** The undo window ran out: the real delete goes ahead. */
const closeUndoToast = () => act(async () => { toasts.at(-1)?.onAutoClose?.() })
const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
const row = (id: number): Transcript => ({ id, content: `Transcript ${id}`, createdAt: `2026-09-02T12:00:${String(id % 60).padStart(2, '0')}Z`, wordCount: 2, durationSeconds: 1, targetApp: null, pinned: false, audioPath: null, sourceName: null, model: null, pending: false, revision: 0 })
const pages = (rows: Transcript[]) => ({ pages: [{ rows }], pageParams: [{ cursorCreatedAt: null, cursorId: null }] })
const ids = (data?: { pages: { rows: Transcript[] }[] }) => data?.pages.flatMap(p => p.rows).map(t => t.id)
const feedKey = queryKeys.transcripts(NO_FILTERS)
beforeEach(() => { queryClient.clear(); toasts.length = 0; vi.mocked(invoke).mockReset().mockResolvedValue([]) })
afterEach(() => { cleanup(); queryClient.clear(); onlineManager.setOnline(true) })

describe('local Query cache', () => {
  it('fetches only the search when the underlying feed is disabled', async () => {
    const { result } = renderHook(() => {
      useTranscripts(NO_FILTERS, false)
      return { ...useTranscriptSearch('hello', NO_FILTERS) }
    }, { wrapper })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(invoke).toHaveBeenCalledTimes(1)
    expect(vi.mocked(invoke).mock.calls[0][0]).toBe('search_transcripts')
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
})

describe('deleting behind Undo', () => {
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
    // Ids of its own: a committed delete stays hidden for the session.
    queryClient.setQueryData(feedKey, pages([row(6), row(5)]))
    const { result } = renderHook(() => ({ feed: useTranscripts(NO_FILTERS), del: useDeleteTranscript() }), { wrapper })
    act(() => result.current.del(6))
    const input = document.body.appendChild(document.createElement('input'))
    act(() => { input.dispatchEvent(new KeyboardEvent('keydown', { key: 'z', ctrlKey: true, bubbles: true })) })
    input.remove()
    expect(ids(result.current.feed.data)).toEqual([5])
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
  })

  it('keeps a deleted transcript hidden when History reopens on the list cached before the delete', async () => {
    queryClient.setQueryData(feedKey, pages([row(8), row(7)]))
    const { result } = renderHook(() => useDeleteTranscript(), { wrapper })
    const feed = renderHook(() => useTranscripts(NO_FILTERS), { wrapper })
    act(() => result.current(8))
    feed.unmount()
    await closeUndoToast()
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === 'delete_transcript')).toBe(true)

    const reopened = renderHook(() => useTranscripts(NO_FILTERS), { wrapper })
    expect(ids(reopened.result.current.data)).toEqual([7])
  })

  it('keeps a pending dictionary delete hidden when the list refetches', async () => {
    const entry = (id: number): DictionaryResponse => ({ id, term: `term ${id}`, replacement: `fix ${id}`, hits: 0, createdAt: '2026-09-02T12:00:00Z' })
    vi.mocked(invoke).mockResolvedValue([entry(1), entry(2)])
    const { result } = renderHook(() => ({ list: useDictionary(), del: useDeleteDictionaryEntry() }), { wrapper })
    await waitFor(() => expect(result.current.list.data).toHaveLength(2))
    act(() => result.current.del(entry(2)))
    await act(async () => { await result.current.list.refetch() })
    expect(result.current.list.data?.map(e => e.id)).toEqual([1])
  })
})

describe('saves', () => {
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
})
