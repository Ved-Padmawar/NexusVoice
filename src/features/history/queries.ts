import {
  keepPreviousData,
  useInfiniteQuery,
  useMutation,
  useQuery,
  type InfiniteData,
} from '@tanstack/react-query'
import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import { commands, type PageQuery } from '../../shared/bindings'
import { createPendingDeletes, deleteWithUndo } from '../../shared/deleteWithUndo'
import { extractErrorMessage } from '../../shared/errors'
import { queryClient } from '../../app/queryClient'
import { revisionOf, useRevision } from '../../app/mirror'
import { queryKeys, type TranscriptFilters } from '../../shared/queryKeys'
import type { Transcript } from '../../shared/types'
import { localDateKey } from '../../shared/utils'

export const PAGE_SIZE = 50

export const NO_FILTERS: TranscriptFilters = { from: null, to: null, sortAsc: false }

type Cursor = { cursorCreatedAt: string | null; cursorId: number | null }

/** `next` is fixed at fetch time, so a delete can't make a full page look like the last. */
type TranscriptPage = { rows: Transcript[]; next?: Cursor }

type TranscriptPages = InfiniteData<TranscriptPage, Cursor>

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

/** A read of History at a revision is current for good. A failed one is
 *  retried, soon, so the change it was for still shows. */
const AT_REVISION = { staleTime: Infinity, retry: 2, retryDelay: (failures: number) => 100 * 2 ** failures } as const

/** The newest pages an earlier revision of `list` read. */
function earlierPages(list: readonly unknown[]): TranscriptPages | undefined {
  let newest: { rev: number; pages: TranscriptPages } | undefined
  for (const [key, pages] of queryClient.getQueriesData<TranscriptPages>({ queryKey: list })) {
    const rev = key[list.length] as number
    if (pages && (!newest || rev > newest.rev)) newest = { rev, pages }
  }
  return newest?.pages
}

/** `list` at History revision `rev`. A new revision starts from the pages an
 *  earlier one read, marked stale, so each is read again and the list keeps
 *  its length and scroll position. */
const pagesAt = (list: readonly unknown[], rev: number) => ({
  ...AT_REVISION,
  queryKey: [...list, rev],
  initialData: () => earlierPages(list),
  initialDataUpdatedAt: 0,
  staleTime: ({ state }: { state: { dataUpdatedAt: number } }) => (state.dataUpdatedAt === 0 ? 0 : Infinity),
})

const readPage = (query: string, page: PageQuery) => query ? commands.searchTranscripts(query, page) : commands.getTranscripts(page)

const feedOptions = (query: string, filters: TranscriptFilters, rev: number) => ({
  ...pagesAt(query ? queryKeys.transcriptSearch(query, filters) : queryKeys.transcripts(filters), rev),
  queryFn: ({ pageParam }: { pageParam: Cursor }) => readPage(query, pageArg(filters, pageParam)).then(toPage),
  initialPageParam: NO_CURSOR,
  getNextPageParam,
})

const statsOptions = (rev: number) => ({
  ...AT_REVISION,
  queryKey: queryKeys.stats(rev),
  queryFn: () => commands.getUsageStats(),
})

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
    const rows = await readPage(query, pageArg(filters, cursor, EXPORT_PAGE_SIZE))
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

// A new filter or term keeps the old rows up instead of flashing the skeleton.
export function useHistoryFeed(query: string, filters: TranscriptFilters) {
  const ids = pendingDeletes.useIds()
  const select = useCallback((data: TranscriptPages) => (ids.size ? withoutRows(data, ids) : data), [ids])
  const rev = useRevision('history')
  return useInfiniteQuery({ ...feedOptions(query, filters, rev), placeholderData: keepPreviousData, select })
}

export function useStats() {
  return useQuery({ ...statsOptions(useRevision('history')), placeholderData: keepPreviousData })
}

export type DayWords = { day: Date; words: number }

const localDay = (now = new Date()) => ({ date: localDateKey(now), utcOffset: -now.getTimezoneOffset() })

/** Today's date and UTC offset, refreshed at local midnight and when the
 *  window regains focus (a tray window can stay open across days). */
function useLocalDay() {
  const [day, setDay] = useState(localDay)
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>
    const refresh = () => {
      clearTimeout(timer)
      const now = new Date()
      const next = localDay(now)
      setDay(current => current.date === next.date && current.utcOffset === next.utcOffset ? current : next)
      timer = setTimeout(refresh, +new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1) - +now)
    }
    refresh()
    window.addEventListener('focus', refresh)
    return () => {
      clearTimeout(timer)
      window.removeEventListener('focus', refresh)
    }
  }, [])
  return day
}

/** Words per local day, oldest first, one entry per day: days with no
 *  dictation (which the backend omits) count zero. */
export function useDailyWords(days: number) {
  const { date, utcOffset } = useLocalDay()
  return useQuery({
    ...AT_REVISION,
    queryKey: queryKeys.dailyWords(days, date, utcOffset, useRevision('history')),
    placeholderData: keepPreviousData,
    queryFn: () => commands.getDailyWords(days, utcOffset, date),
    select: (rows): DayWords[] => {
      const byDay = new Map(rows.map(r => [r.day, r.words]))
      const [year, month, today] = date.split('-').map(Number)
      return Array.from({ length: days }, (_, i) => {
        const day = new Date(year, month - 1, today - (days - 1 - i))
        return { day, words: byDay.get(localDateKey(day)) ?? 0 }
      })
    },
  })
}

/** Deletes a dictation behind an Undo toast. */
export function useDeleteTranscript() {
  const { mutateAsync } = useMutation({
    mutationFn: (id: number) => commands.deleteTranscript(id),
    onError: (e) => toast.error(extractErrorMessage(e, 'Failed to delete transcript')),
  })
  return useCallback((id: number) => deleteWithUndo({
    message: 'Dictation deleted',
    id,
    pending: pendingDeletes,
    commit: () => mutateAsync(id),
  }), [mutateAsync])
}

export const setPinned = (id: number, pinned: boolean) => commands.setTranscriptPinned(id, pinned)

// Startup waits on these; a failed read shows in the screen that uses it.
export const prefetchTranscripts = () => queryClient.infiniteQuery(feedOptions('', NO_FILTERS, revisionOf('history'))).catch(() => {})

export const prefetchStats = () => queryClient.query(statsOptions(revisionOf('history'))).catch(() => {})
