import {
  keepPreviousData,
  useInfiniteQuery,
  useMutation,
  useQuery,
  type InfiniteData,
} from '@tanstack/react-query'
import { useCallback } from 'react'
import { toast } from 'sonner'
import { commands, type PageQuery } from '../../shared/bindings'
import { createPendingDeletes, deleteWithUndo } from '../../shared/deleteWithUndo'
import { extractErrorMessage } from '../../shared/errors'
import { queryClient } from '../../app/queryClient'
import { reads } from '../../app/sync'
import { queryKeys, type TranscriptFilters } from '../../shared/queryKeys'
import type { Transcript } from '../../shared/types'

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

const feedOptions = (filters: TranscriptFilters) => ({
  queryKey: queryKeys.transcripts(filters),
  queryFn: ({ pageParam }: { pageParam: Cursor }) => commands.getTranscripts(pageArg(filters, pageParam)).then(toPage),
  initialPageParam: NO_CURSOR,
  getNextPageParam,
  ...reads('history'),
})

const statsOptions = {
  queryKey: queryKeys.stats,
  queryFn: () => commands.getUsageStats(),
  ...reads('history'),
}

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
    queryFn: ({ pageParam }) => commands.searchTranscripts(query, pageArg(filters, pageParam)).then(toPage),
    initialPageParam: NO_CURSOR,
    getNextPageParam,
    enabled: query.length > 0,
    placeholderData: keepPreviousData,
    select,
    ...reads('history'),
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
    ...reads('history'),
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
export const prefetchTranscripts = () => queryClient.infiniteQuery(feedOptions(NO_FILTERS)).catch(() => {})

export const prefetchStats = () => queryClient.query(statsOptions).catch(() => {})
