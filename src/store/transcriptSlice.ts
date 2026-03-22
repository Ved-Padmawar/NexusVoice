import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from '../lib/commands'
import { TranscriptSchema, UsageStatsSchema, DictionaryEntrySchema, type Transcript, type UsageStats } from '../types'
import { invokeWithRefresh } from './invokeWithRefresh'
import { toast } from 'sonner'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'

export type TranscriptSlice = {
  transcripts: Transcript[]
  transcriptOffset: number
  transcriptHasMore: boolean
  filterFrom: string | null
  filterTo: string | null
  filterSortAsc: boolean
  searchQuery: string
  searchResults: Transcript[]
  isSearching: boolean
  stats: UsageStats | null
  isLoading: boolean
  init: () => Promise<void>
  fetchStats: () => Promise<void>
  setFilters: (from: string | null, to: string | null, sortAsc: boolean) => Promise<void>
  loadMoreTranscripts: () => Promise<void>
  searchTranscripts: (query: string) => Promise<void>
  addTranscript: (content: string) => Promise<void>
}

export const createTranscriptSlice: StateCreator<AppState, [], [], TranscriptSlice> = (set, get) => ({
  transcripts: [],
  transcriptOffset: 0,
  transcriptHasMore: true,
  filterFrom: null,
  filterTo: null,
  filterSortAsc: false,
  searchQuery: '',
  searchResults: [],
  isSearching: false,
  stats: null,
  isLoading: false,

  init: async () => {
    if (!get().user) return
    set({ isLoading: true, transcriptOffset: 0, transcriptHasMore: true, filterFrom: null, filterTo: null, filterSortAsc: false, searchQuery: '', searchResults: [] })
    try {
      const [rawTranscripts, rawDictionary, hotkeys] = await Promise.all([
        invokeWithRefresh<unknown>(COMMANDS.GET_TRANSCRIPTS, { limit: 50, offset: 0, from: null, to: null, sortAsc: false }),
        invokeWithRefresh<unknown>(COMMANDS.GET_DICTIONARY),
        invoke<string[]>(COMMANDS.GET_REGISTERED_HOTKEYS),
      ])
      const transcripts = TranscriptSchema.array().parse(rawTranscripts)
      const dictionary = DictionaryEntrySchema.array().parse(rawDictionary)
      invokeWithRefresh<unknown>(COMMANDS.GET_USAGE_STATS)
        .then(raw => set({ stats: UsageStatsSchema.parse(raw) }))
        .catch(e => console.warn('[store] get_usage_stats failed:', e))
      set({ transcripts, transcriptOffset: transcripts.length, transcriptHasMore: transcripts.length === 50, dictionary, hasHotkey: hotkeys.length > 0 })
    } catch (e) {
      toast.error(e instanceof Error ? e.message : 'Failed to load data')
    } finally {
      set({ isLoading: false })
    }
  },

  fetchStats: async () => {
    try {
      const raw = await invokeWithRefresh<unknown>(COMMANDS.GET_USAGE_STATS)
      set({ stats: UsageStatsSchema.parse(raw) })
    } catch {
      set({ stats: null })
    }
  },

  setFilters: async (from, to, sortAsc) => {
    set({ filterFrom: from, filterTo: to, filterSortAsc: sortAsc, transcriptOffset: 0, transcriptHasMore: true, transcripts: [] })
    try {
      const items = TranscriptSchema.array().parse(
        await invokeWithRefresh<unknown>(COMMANDS.GET_TRANSCRIPTS, { limit: 50, offset: 0, from, to, sortAsc })
      )
      set({ transcripts: items, transcriptOffset: items.length, transcriptHasMore: items.length === 50 })
    } catch { /* ignore */ }
    const { searchQuery } = get()
    if (searchQuery.trim()) {
      set({ isSearching: true })
      try {
        const results = TranscriptSchema.array().parse(
          await invokeWithRefresh<unknown>(COMMANDS.SEARCH_TRANSCRIPTS, { query: searchQuery, limit: 50, offset: 0, from, to, sortAsc })
        )
        set({ searchResults: results })
      } catch { set({ searchResults: [] }) }
      finally { set({ isSearching: false }) }
    }
  },

  loadMoreTranscripts: async () => {
    const { transcriptOffset, transcriptHasMore, transcripts, filterFrom, filterTo, filterSortAsc } = get()
    if (!transcriptHasMore) return
    try {
      const more = TranscriptSchema.array().parse(
        await invokeWithRefresh<unknown>(COMMANDS.GET_TRANSCRIPTS, { limit: 50, offset: transcriptOffset, from: filterFrom, to: filterTo, sortAsc: filterSortAsc })
      )
      set({
        transcripts: [...transcripts, ...more],
        transcriptOffset: transcriptOffset + more.length,
        transcriptHasMore: more.length === 50,
      })
    } catch { /* ignore */ }
  },

  searchTranscripts: async (query: string) => {
    set({ searchQuery: query })
    if (!query.trim()) {
      set({ searchResults: [], isSearching: false })
      return
    }
    set({ isSearching: true })
    const { filterFrom, filterTo, filterSortAsc } = get()
    try {
      const results = TranscriptSchema.array().parse(
        await invokeWithRefresh<unknown>(COMMANDS.SEARCH_TRANSCRIPTS, { query, limit: 50, offset: 0, from: filterFrom, to: filterTo, sortAsc: filterSortAsc })
      )
      set({ searchResults: results })
    } catch {
      set({ searchResults: [] })
    } finally {
      set({ isSearching: false })
    }
  },

  addTranscript: async (content) => {
    try {
      const newTranscript = TranscriptSchema.parse(
        await invokeWithRefresh<unknown>(COMMANDS.SAVE_TRANSCRIPT, { content })
      )
      set((state) => ({ transcripts: [newTranscript, ...state.transcripts] }))
    } catch (e) {
      toast.error(e instanceof Error ? e.message : 'Failed to save transcript')
    }
  },
})
