import { z } from 'zod'
import { COMMANDS } from '../lib/commands'
import { TranscriptSchema, UsageStatsSchema, type Transcript, type UsageStats } from '../types'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'
import type { AsyncStatus } from './asyncStatus'
import { extractErrorMessage } from '../lib/errors'

/// A full page implies more may exist; a short page is the last.
export const PAGE_SIZE = 50

/// Keyset cursor: resume strictly after the last loaded row. Stable across inserts
/// and deletes, which offsets are not.
const cursorOf = (rows: Transcript[]) => {
  const last = rows.at(-1)
  return last ? { cursorCreatedAt: last.createdAt, cursorId: last.id } : { cursorCreatedAt: null, cursorId: null }
}

export type TranscriptSlice = {
  transcripts: Transcript[]
  transcriptHasMore: boolean
  transcriptLoadingMore: boolean
  transcriptsStatus: AsyncStatus
  transcriptsError: string | null
  filterFrom: string | null
  filterTo: string | null
  filterSortAsc: boolean
  searchQuery: string
  searchResults: Transcript[]
  isSearching: boolean
  stats: UsageStats | null
  statsStatus: AsyncStatus
  statsError: string | null
  loadTranscripts: () => Promise<void>
  loadStats: () => Promise<void>
  retryTranscripts: () => Promise<void>
  setFilters: (from: string | null, to: string | null, sortAsc: boolean) => Promise<void>
  loadMoreTranscripts: () => Promise<void>
  searchTranscripts: (query: string) => Promise<void>
  addTranscript: (content: string) => Promise<void>
  deleteTranscript: (id: number) => Promise<void>
}

export const createTranscriptSlice: StateCreator<AppState, [], [], TranscriptSlice> = (set, get) => ({
  transcripts: [],
  transcriptHasMore: true,
  transcriptLoadingMore: false,
  transcriptsStatus: 'idle',
  transcriptsError: null,
  filterFrom: null,
  filterTo: null,
  filterSortAsc: false,
  searchQuery: '',
  searchResults: [],
  isSearching: false,
  stats: null,
  statsStatus: 'idle',
  statsError: null,

  loadTranscripts: async () => {
    if (!get().user) return
    set({
      transcriptsStatus: 'loading',
      transcriptsError: null,
      transcriptHasMore: true,
      transcriptLoadingMore: false,
      filterFrom: null,
      filterTo: null,
      filterSortAsc: false,
      searchQuery: '',
      searchResults: [],
    })
    try {
      const transcripts = z.array(TranscriptSchema).parse(
        await invoke<unknown>(COMMANDS.GET_TRANSCRIPTS, { page: { limit: PAGE_SIZE, cursorCreatedAt: null, cursorId: null, from: null, to: null, sortAsc: false } })
      )
      set({
        transcripts,
        transcriptHasMore: transcripts.length === PAGE_SIZE,
        transcriptsStatus: 'success',
      })
    } catch (e) {
      const message = extractErrorMessage(e, 'Failed to load transcripts')
      set({ transcriptsStatus: 'error', transcriptsError: message })
    }
  },

  loadStats: async () => {
    if (!get().user) return
    set({ statsStatus: 'loading', statsError: null })
    try {
      const raw = await invoke<unknown>(COMMANDS.GET_USAGE_STATS)
      set({ stats: UsageStatsSchema.parse(raw), statsStatus: 'success' })
    } catch (e) {
      const message = extractErrorMessage(e, 'Failed to load usage stats')
      set({ stats: null, statsStatus: 'error', statsError: message })
    }
  },

  setFilters: async (from, to, sortAsc) => {
    set({ filterFrom: from, filterTo: to, filterSortAsc: sortAsc, transcriptHasMore: true, transcriptLoadingMore: false, transcripts: [], transcriptsStatus: 'loading', transcriptsError: null })
    try {
      const items = z.array(TranscriptSchema).parse(
        await invoke<unknown>(COMMANDS.GET_TRANSCRIPTS, { page: { limit: PAGE_SIZE, cursorCreatedAt: null, cursorId: null, from, to, sortAsc } })
      )
      set({ transcripts: items, transcriptHasMore: items.length === PAGE_SIZE, transcriptsStatus: 'success' })
    } catch (e) {
      set({ transcriptsStatus: 'error', transcriptsError: extractErrorMessage(e, 'Failed to load transcripts') })
    }
    const { searchQuery } = get()
    if (searchQuery.trim()) {
      await get().searchTranscripts(searchQuery)
    }
  },

  // The scroll sentinel refires while visible, so admit one fetch at a time.
  loadMoreTranscripts: async () => {
    const { transcripts, transcriptHasMore, transcriptLoadingMore, filterFrom, filterTo, filterSortAsc } = get()
    if (!transcriptHasMore || transcriptLoadingMore) return
    set({ transcriptLoadingMore: true })
    try {
      const more = z.array(TranscriptSchema).parse(
        await invoke<unknown>(COMMANDS.GET_TRANSCRIPTS, { page: { limit: PAGE_SIZE, ...cursorOf(transcripts), from: filterFrom, to: filterTo, sortAsc: filterSortAsc } })
      )
      set((state) => {
        const seen = new Set(state.transcripts.map(t => t.id))
        return {
          transcripts: [...state.transcripts, ...more.filter(t => !seen.has(t.id))],
          transcriptHasMore: more.length === PAGE_SIZE,
          transcriptLoadingMore: false,
        }
      })
    } catch (e) {
      // Don't wipe the loaded feed — surface a transient error.
      set({ transcriptLoadingMore: false })
      toast.error(extractErrorMessage(e, 'Failed to load more transcripts'))
    }
  },

  searchTranscripts: async (query: string) => {
    set({ searchQuery: query })
    if (!query.trim()) {
      set({ searchResults: [], isSearching: false, transcriptsStatus: 'success', transcriptsError: null })
      return
    }
    set({ isSearching: true, transcriptsStatus: 'loading', transcriptsError: null })
    const { filterFrom, filterTo, filterSortAsc } = get()
    try {
      const results = z.array(TranscriptSchema).parse(
        await invoke<unknown>(COMMANDS.SEARCH_TRANSCRIPTS, { query, page: { limit: PAGE_SIZE, cursorCreatedAt: null, cursorId: null, from: filterFrom, to: filterTo, sortAsc: filterSortAsc } })
      )
      set({ searchResults: results, transcriptsStatus: 'success' })
    } catch (e) {
      set({ searchResults: [], transcriptsStatus: 'error', transcriptsError: extractErrorMessage(e, 'Search failed') })
    } finally {
      set({ isSearching: false })
    }
  },

  // Re-run the *current* view (search if active, else filtered list) — used by the
  // feed's error-retry so it doesn't reset filters/search like loadTranscripts does.
  retryTranscripts: async () => {
    const { searchQuery, filterFrom, filterTo, filterSortAsc } = get()
    if (searchQuery.trim()) {
      await get().searchTranscripts(searchQuery)
    } else {
      await get().setFilters(filterFrom, filterTo, filterSortAsc)
    }
  },

  addTranscript: async (content) => {
    try {
      const newTranscript = TranscriptSchema.parse(
        await invoke<unknown>(COMMANDS.SAVE_TRANSCRIPT, { content })
      )
      set((state) => ({ transcripts: [newTranscript, ...state.transcripts] }))
    } catch (e) {
      toast.error(extractErrorMessage(e, 'Failed to save transcript'))
    }
  },

  deleteTranscript: async (id) => {
    set((state) => ({
      transcripts: state.transcripts.filter(t => t.id !== id),
      searchResults: state.searchResults.filter(t => t.id !== id),
    }))
    try {
      await invoke<boolean>(COMMANDS.DELETE_TRANSCRIPT, { id })
      get().loadStats()
    } catch (e) {
      toast.error(extractErrorMessage(e, 'Failed to delete transcript'))
    }
  },
})
