import { invoke } from '@tauri-apps/api/core'
import {
  keepPreviousData,
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
  type InfiniteData,
} from '@tanstack/react-query'
import { toast } from 'sonner'
import { COMMANDS } from '../commands'
import { extractErrorMessage } from '../errors'
import { queryClient } from './client'
import { queryKeys } from './keys'
import type { Transcript, UsageStats } from '../../types'

export const PAGE_SIZE = 50

export type TranscriptFilters = {
  from: string | null
  to: string | null
  sortAsc: boolean
}

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

const pageArg = (filters: TranscriptFilters, cursor: Cursor) => ({
  limit: PAGE_SIZE,
  ...cursor,
  ...filters,
})

const fetchFeed = (filters: TranscriptFilters, cursor: Cursor) =>
  invoke<Transcript[]>(COMMANDS.GET_TRANSCRIPTS, { page: pageArg(filters, cursor) }).then(toPage)

const fetchSearch = (query: string, filters: TranscriptFilters, cursor: Cursor) =>
  invoke<Transcript[]>(COMMANDS.SEARCH_TRANSCRIPTS, { query, page: pageArg(filters, cursor) }).then(toPage)

// Kept current by `transcript:new` and mutations, so never stale on a timer.
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
  queryFn: () => invoke<UsageStats>(COMMANDS.GET_USAGE_STATS),
  ...EVENT_DRIVEN,
}

/** Closing the main window only hides it; don't refetch for a page nobody sees. */
const refetchType = () => (document.hidden ? 'none' : 'active')

// A new filter or term keeps the old rows up instead of flashing the skeleton.
export function useTranscripts(filters: TranscriptFilters, enabled = true) {
  return useInfiniteQuery({ ...feedOptions(filters), enabled, placeholderData: keepPreviousData })
}

export function useTranscriptSearch(query: string, filters: TranscriptFilters) {
  return useInfiniteQuery({
    queryKey: queryKeys.transcriptSearch(query, filters),
    queryFn: ({ pageParam }) => fetchSearch(query, filters, pageParam),
    initialPageParam: NO_CURSOR,
    getNextPageParam,
    enabled: query.length > 0,
    placeholderData: keepPreviousData,
  })
}

export function useStats() {
  return useQuery(statsOptions)
}

export function useDeleteTranscript() {
  const client = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => invoke<void>(COMMANDS.DELETE_TRANSCRIPT, { id }),
    // Drop the row in place rather than refetch every page. A fetch in flight
    // would restore it, so cancel and re-run those.
    onSuccess: async (_, id) => {
      const root = { queryKey: queryKeys.transcriptsRoot }
      const inFlight = client.getQueryCache().findAll({ ...root, fetchStatus: 'fetching' })
      await client.cancelQueries(root)
      client.setQueriesData<TranscriptPages>(root, (data) =>
        data && { ...data, pages: data.pages.map(page => ({ ...page, rows: page.rows.filter(row => row.id !== id) })) })
      inFlight.forEach(query => void client.invalidateQueries({ queryKey: query.queryKey, exact: true }))
      void client.invalidateQueries({ queryKey: queryKeys.stats })
    },
    onError: (e) => toast.error(extractErrorMessage(e, 'Failed to delete transcript')),
  })
}

export const prefetchTranscripts = () =>
  queryClient.prefetchInfiniteQuery(feedOptions(NO_FILTERS))

export const prefetchStats = () => queryClient.prefetchQuery(statsOptions)

export async function addTranscript(transcript: Transcript) {
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
}
