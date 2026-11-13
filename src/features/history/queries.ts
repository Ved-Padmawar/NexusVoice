import {
  keepPreviousData,
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
  type InfiniteData,
} from '@tanstack/react-query'
import { useCallback } from 'react'
import { toast } from 'sonner'
import { commands, type PageQuery } from '../../shared/bindings'
import { createPendingDeletes, deleteWithUndo } from '../../shared/deleteWithUndo'
import { extractErrorMessage } from '../../shared/errors'
import { queryClient } from '../../app/queryClient'
import { queryKeys, type TranscriptFilters } from '../../shared/queryKeys'
import type { Transcript } from '../../shared/types'

export const PAGE_SIZE = 50

export const NO_FILTERS: TranscriptFilters = { from: null, to: null, sortAsc: false }

type Cursor = { cursorCreatedAt: string | null; cursorId: number | null }

/** `next` is fixed at fetch time, so a delete can't make a full page look like the last. */
export type TranscriptPage = { rows: Transcript[]; next?: Cursor }

export type TranscriptPages = InfiniteData<TranscriptPage, Cursor>

const NO_CURSOR: Cursor = { cursorCreatedAt: null, cursorId: null }

const toPage = (rows: Transcript[]): TranscriptPage => {
  const last = rows.at(-1)
  return {
    rows,
    next: last && rows.length >= PAGE_SIZE ? { cursorCreatedAt: last.createdAt, cursorId: last.id } : undefined,
  }
}

const getNextPageParam = (lastPage: TranscriptPage) => lastPage.next

const pageArg = (filters: TranscriptFilters, cursor: Cursor, limit = PAGE_SIZE): PageQuery => ({
  limit,
  ...cursor,
  ...filters,
})

const EXPORT_PAGE_SIZE = 200


const fetchFeed = (filters: TranscriptFilters, cursor: Cursor) =>
  commands.getTranscripts(pageArg(filters, cursor)).then(toPage)

const fetchSearch = (query: string, filters: TranscriptFilters, cursor: Cursor) =>
  commands.searchTranscripts(query, pageArg(filters, cursor)).then(toPage)

// Kept current by `transcriptSaved` and mutations, so never stale on a timer.
// Invalidations skipped while hidden refetch when the window is shown.
const EVENT_DRIVEN = { staleTime: Infinity, refetchOnWindowFocus: true } as const

const feedOptions = (filters: TranscriptFilters) => ({
  queryKey: queryKeys.transcripts(filters),
  queryFn: ({ pageParam }: { pageParam: Cursor }) => fetchFeed(filters, pageParam),
  initialPageParam: NO_CURSOR,
  getNextPageParam,
  ...EVENT_DRIVEN,
})

const statsOptions = {
  queryKey: queryKeys.stats,
  queryFn: () => commands.getUsageStats(),
  ...EVENT_DRIVEN,
}

/** Closing the main window only hides it; don't refetch for a page nobody sees. */
const refetchType = () => (document.hidden ? 'none' : 'active')

const pendingDeletes = createPendingDeletes()

/** Every transcript the feed shows for this search and filters, a page at a
 *  time so no call holds the whole table. A row hidden for Undo at any point
 *  stays out, even once its delete commits, and so does an import with no
 *  text yet. */
export async function fetchAllTranscripts(query: string, filters: TranscriptFilters): Promise<Transcript[]> {
  const all: Transcript[] = []
  const hidden = new Set<number>()
  const noteHidden = () => pendingDeletes.current().forEach(id => hidden.add(id))
  let cursor = NO_CURSOR
  for (;;) {
    noteHidden()
    const page = pageArg(filters, cursor, EXPORT_PAGE_SIZE)
    const rows = await (query ? commands.searchTranscripts(query, page) : commands.getTranscripts(page))
    all.push(...rows)
    const last = rows.at(-1)
    if (!last || rows.length < EXPORT_PAGE_SIZE) {
      noteHidden()
      return all.filter(row => !row.pending && !hidden.has(row.id))
    }
    cursor = { cursorCreatedAt: last.createdAt, cursorId: last.id }
  }
}

const withoutRows = (data: TranscriptPages, ids: ReadonlySet<number>): TranscriptPages =>
  ({ ...data, pages: data.pages.map(page => ({ ...page, rows: page.rows.filter(row => !ids.has(row.id)) })) })

function useHidePendingDeletes() {
  const ids = pendingDeletes.useIds()
  return useCallback((data: TranscriptPages) => (ids.size ? withoutRows(data, ids) : data), [ids])
}

// A new filter or term keeps the old rows up instead of flashing the skeleton.
export function useTranscripts(filters: TranscriptFilters, enabled = true) {
  const select = useHidePendingDeletes()
  return useInfiniteQuery({ ...feedOptions(filters), enabled, placeholderData: keepPreviousData, select })
}

export function useTranscriptSearch(query: string, filters: TranscriptFilters) {
  const select = useHidePendingDeletes()
  return useInfiniteQuery({
    queryKey: queryKeys.transcriptSearch(query, filters),
    queryFn: ({ pageParam }) => fetchSearch(query, filters, pageParam),
    initialPageParam: NO_CURSOR,
    getNextPageParam,
    enabled: query.length > 0,
    placeholderData: keepPreviousData,
    select,
    // Same contract as the feed: `addTranscript` may skip the refetch while hidden.
    ...EVENT_DRIVEN,
  })
}

export function useStats() {
  return useQuery(statsOptions)
}

export type DayWords = { day: Date; words: number }

/** Words per local day, oldest first, one entry per day: days with no
 *  dictation (which the backend omits) count zero. */
export function useDailyWords(days: number) {
  return useQuery({
    queryKey: queryKeys.dailyWords(days),
    queryFn: () => commands.getDailyWords(days, -new Date().getTimezoneOffset()),
    select: (rows): DayWords[] => {
      const byDay = new Map(rows.map(r => [r.day, r.words]))
      const today = new Date()
      return Array.from({ length: days }, (_, i) => {
        const day = new Date(today.getFullYear(), today.getMonth(), today.getDate() - (days - 1 - i))
        const key = `${day.getFullYear()}-${String(day.getMonth() + 1).padStart(2, '0')}-${String(day.getDate()).padStart(2, '0')}`
        return { day, words: byDay.get(key) ?? 0 }
      })
    },
    ...EVENT_DRIVEN,
  })
}

let feedWrites: Promise<unknown> = Promise.resolve()

/** Run a feed write after every earlier one, so the writes for one entry
 *  apply in the order its events came, whatever each awaits first. */
function inOrder(write: () => Promise<void>) {
  const run = feedWrites.then(write)
  feedWrites = run.catch(() => {})
  return run
}

/** Change every cached feed and search in place rather than refetch every
 *  page. A fetch in flight would land older rows over the change, so those are
 *  cancelled first and run again after. */
const writeRows = (update: (data: TranscriptPages) => TranscriptPages) => inOrder(async () => {
  const root = { queryKey: queryKeys.transcriptsRoot }
  const inFlight = queryClient.getQueryCache().findAll({ ...root, fetchStatus: 'fetching' })
  await queryClient.cancelQueries(root)
  queryClient.setQueriesData<TranscriptPages>(root, (data) => data && update(data))
  inFlight.forEach(query => void queryClient.invalidateQueries({ queryKey: query.queryKey, exact: true }))
})

export const dropRow = (id: number) => writeRows(data => withoutRows(data, new Set([id])))

/** Deletes a dictation behind an Undo toast. */
export function useDeleteTranscript() {
  const client = useQueryClient()
  const { mutateAsync } = useMutation({
    mutationFn: (id: number) => commands.deleteTranscript(id),
    onSuccess: (_, id) => {
      void client.invalidateQueries({ queryKey: queryKeys.stats })
      return dropRow(id)
    },
    onError: (e) => toast.error(extractErrorMessage(e, 'Failed to delete transcript')),
  })
  return useCallback((id: number) => deleteWithUndo({
    message: 'Dictation deleted',
    id,
    pending: pendingDeletes,
    commit: () => mutateAsync(id),
  }), [mutateAsync])
}

/** Change the entries in `changes` in every cached feed and search, in one
 *  write. Only their rows get new objects, so the other rows' memos hold,
 *  unlike a refetch. */
export const patchRows = (changes: ReadonlyMap<number, (row: Transcript) => Transcript>) => writeRows(data => ({
  ...data,
  pages: data.pages.map(page => page.rows.some(row => changes.has(row.id))
    ? { ...page, rows: page.rows.map(row => changes.get(row.id)?.(row) ?? row) }
    : page),
}))

export const patchRow = (id: number, change: (row: Transcript) => Transcript) => patchRows(new Map([[id, change]]))

/** Show an entry whose text changed. Feeds take it in place; searches run
 *  again, since the new text may match differently. Its word count moves the stats. */
export async function showRewritten(entry: Transcript) {
  await patchRow(entry.id, () => entry)
  void queryClient.invalidateQueries({ queryKey: queryKeys.transcriptSearchRoot, refetchType: refetchType() })
  void queryClient.invalidateQueries({ queryKey: queryKeys.stats, refetchType: refetchType() })
}

/** Pin or unpin an entry, and show it on every cached feed and search at once. */
export async function setPinned(id: number, pinned: boolean) {
  await commands.setTranscriptPinned(id, pinned)
  await patchRow(id, row => ({ ...row, pinned }))
}

export const prefetchTranscripts = () =>
  queryClient.prefetchInfiniteQuery(feedOptions(NO_FILTERS))

export const prefetchStats = () => queryClient.prefetchQuery(statsOptions)

export const addTranscript = (transcript: Transcript) => inOrder(async () => {
  const queryKey = queryKeys.transcripts(NO_FILTERS)
  // An older response must not overwrite the event's newer database state.
  await queryClient.cancelQueries({ queryKey, exact: true })
  queryClient.setQueryData<TranscriptPages>(queryKey, (data) => {
    if (!data) return data
    if (data.pages.some(page => page.rows.some(row => row.id === transcript.id))) return data
    // Grow, don't trim: the next page's cursor is fixed, so a trimmed row would vanish.
    const [first = { rows: [] }, ...rest] = data.pages
    return { ...data, pages: [{ ...first, rows: [transcript, ...first.rows] }, ...rest] }
  })
  const feed = queryClient.getQueryCache().find({ queryKey, exact: true })
  // The default feed is already updated. Other filters/searches must be
  // re-evaluated by the backend; inactive views only need marking stale.
  void queryClient.invalidateQueries({
    queryKey: queryKeys.transcriptsRoot,
    predicate: query => query !== feed || !feed.state.data,
    refetchType: refetchType(),
  })
  void queryClient.invalidateQueries({ queryKey: queryKeys.stats, refetchType: refetchType() })
})
