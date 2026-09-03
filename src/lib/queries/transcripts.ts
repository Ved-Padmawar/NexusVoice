import { invoke } from '@tauri-apps/api/core'
import {
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

export type TranscriptPages = InfiniteData<Transcript[], Cursor>

const NO_CURSOR: Cursor = { cursorCreatedAt: null, cursorId: null }

const cursorAfter = (rows: Transcript[]): Cursor => {
  const last = rows.at(-1)
  return last ? { cursorCreatedAt: last.createdAt, cursorId: last.id } : NO_CURSOR
}

const getNextPageParam = (lastPage: Transcript[]) =>
  lastPage.length >= PAGE_SIZE ? cursorAfter(lastPage) : undefined

const pageArg = (filters: TranscriptFilters, cursor: Cursor) => ({
  limit: PAGE_SIZE,
  ...cursor,
  ...filters,
})

const fetchFeed = (filters: TranscriptFilters, cursor: Cursor) =>
  invoke<Transcript[]>(COMMANDS.GET_TRANSCRIPTS, { page: pageArg(filters, cursor) })

const fetchSearch = (query: string, filters: TranscriptFilters, cursor: Cursor) =>
  invoke<Transcript[]>(COMMANDS.SEARCH_TRANSCRIPTS, { query, page: pageArg(filters, cursor) })

const feedOptions = (filters: TranscriptFilters) => ({
  queryKey: queryKeys.transcripts(filters),
  queryFn: ({ pageParam }: { pageParam: Cursor }) => fetchFeed(filters, pageParam),
  initialPageParam: NO_CURSOR,
  getNextPageParam,
})

const statsOptions = {
  queryKey: queryKeys.stats,
  queryFn: () => invoke<UsageStats>(COMMANDS.GET_USAGE_STATS),
}

export function useTranscripts(filters: TranscriptFilters, enabled = true) {
  return useInfiniteQuery({ ...feedOptions(filters), enabled })
}

export function useTranscriptSearch(query: string, filters: TranscriptFilters) {
  return useInfiniteQuery({
    queryKey: queryKeys.transcriptSearch(query, filters),
    queryFn: ({ pageParam }) => fetchSearch(query, filters, pageParam),
    initialPageParam: NO_CURSOR,
    getNextPageParam,
    enabled: query.length > 0,
  })
}

export function useStats() {
  return useQuery(statsOptions)
}

export function useDeleteTranscript() {
  const client = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => invoke<void>(COMMANDS.DELETE_TRANSCRIPT, { id }),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: queryKeys.transcriptsRoot })
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
    if (data.pages.some(page => page.some(row => row.id === transcript.id))) return data
    const [first = [], ...rest] = data.pages
    const grown = [transcript, ...first]
    return { ...data, pages: [rest.length > 0 ? grown.slice(0, PAGE_SIZE) : grown, ...rest] }
  })
  const feed = queryClient.getQueryCache().find({ queryKey, exact: true })
  // The default feed is already updated. Other filters/searches must be
  // re-evaluated by the backend; inactive views only need marking stale.
  void queryClient.invalidateQueries({
    queryKey: queryKeys.transcriptsRoot,
    predicate: query => query !== feed || !feed.state.data,
  })
  void queryClient.invalidateQueries({ queryKey: queryKeys.stats })
}
