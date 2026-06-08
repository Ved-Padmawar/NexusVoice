import { z } from 'zod'
import { COMMANDS } from '../lib/commands'
import { TranscriptSchema, UsageStatsSchema, type Transcript, type UsageStats } from '../types'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'
import type { AsyncStatus } from './asyncStatus'

export type TranscriptSlice = {
  transcripts: Transcript[]
  transcriptOffset: number
  transcriptHasMore: boolean
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
  transcriptOffset: 0,
  transcriptHasMore: true,
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
      transcriptOffset: 0,
      transcriptHasMore: true,
      filterFrom: null,
      filterTo: null,
      filterSortAsc: false,
      searchQuery: '',
      searchResults: [],
    })
    try {
      const transcripts = z.array(TranscriptSchema).parse(
        await invoke<unknown>(COMMANDS.GET_TRANSCRIPTS, { limit: 50, offset: 0, from: null, to: null, sortAsc: false })
      )
      set({
        transcripts,
        transcriptOffset: transcripts.length,
        transcriptHasMore: transcripts.length === 50,
        transcriptsStatus: 'success',
      })
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Failed to load transcripts'
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
      const message = e instanceof Error ? e.message : 'Failed to load usage stats'
      set({ stats: null, statsStatus: 'error', statsError: message })
    }
  },

  setFilters: async (from, to, sortAsc) => {
    set({ filterFrom: from, filterTo: to, filterSortAsc: sortAsc, transcriptOffset: 0, transcriptHasMore: true, transcripts: [], transcriptsStatus: 'loading', transcriptsError: null })
    try {
      const items = z.array(TranscriptSchema).parse(
        await invoke<unknown>(COMMANDS.GET_TRANSCRIPTS, { limit: 50, offset: 0, from, to, sortAsc })
      )
      set({ transcripts: items, transcriptOffset: items.length, transcriptHasMore: items.length === 50, transcriptsStatus: 'success' })
    } catch (e) {
      set({ transcriptsStatus: 'error', transcriptsError: e instanceof Error ? e.message : 'Failed to load transcripts' })
    }
    const { searchQuery } = get()
    if (searchQuery.trim()) {
      await get().searchTranscripts(searchQuery)
    }
  },

  loadMoreTranscripts: async () => {
    const { transcriptOffset, transcriptHasMore, transcripts, filterFrom, filterTo, filterSortAsc } = get()
    if (!transcriptHasMore) return
    try {
      const more = z.array(TranscriptSchema).parse(
        await invoke<unknown>(COMMANDS.GET_TRANSCRIPTS, { limit: 50, offset: transcriptOffset, from: filterFrom, to: filterTo, sortAsc: filterSortAsc })
      )
      set({
        transcripts: [...transcripts, ...more],
        transcriptOffset: transcriptOffset + more.length,
        transcriptHasMore: more.length === 50,
      })
    } catch (e) {
      // Pagination append: don't wipe the loaded feed — surface a transient error.
      toast.error(e instanceof Error ? e.message : 'Failed to load more transcripts')
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
        await invoke<unknown>(COMMANDS.SEARCH_TRANSCRIPTS, { query, limit: 50, offset: 0, from: filterFrom, to: filterTo, sortAsc: filterSortAsc })
      )
      set({ searchResults: results, transcriptsStatus: 'success' })
    } catch (e) {
      set({ searchResults: [], transcriptsStatus: 'error', transcriptsError: e instanceof Error ? e.message : 'Search failed' })
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
      toast.error(e instanceof Error ? e.message : 'Failed to save transcript')
    }
  },

  deleteTranscript: async (id) => {
    set((state) => ({
      transcripts: state.transcripts.filter(t => t.id !== id),
      searchResults: state.searchResults.filter(t => t.id !== id),
      transcriptOffset: Math.max(0, state.transcriptOffset - 1),
    }))
    try {
      await invoke<boolean>(COMMANDS.DELETE_TRANSCRIPT, { id })
      get().loadStats()
    } catch (e) {
      toast.error(e instanceof Error ? e.message : 'Failed to delete transcript')
    }
  },
})
