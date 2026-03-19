import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

type TokenPairResponse = { accessToken: string; refreshToken: string; expiresInSeconds: number }

function isTokenExpired(e: unknown): boolean {
  return typeof e === 'object' && e !== null && 'code' in e && (e as { code: string }).code === 'token_expired'
}

// Wraps invoke — if token_expired, refreshes once then retries; logs out if refresh also fails
async function invokeWithRefresh<T>(
  cmd: string,
  args?: Record<string, unknown>
): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (e) {
    if (!isTokenExpired(e)) throw e
    // Attempt silent refresh
    const stored = useAppStore.getState().refreshToken
    if (!stored) {
      useAppStore.getState().logout()
      throw e
    }
    try {
      const pair = await invoke<TokenPairResponse>('refresh_token', { refreshToken: stored })
      await invoke('store_refresh_token', {
        refreshToken: pair.refreshToken,
        userId: useAppStore.getState().user?.id,
        accessToken: pair.accessToken,
      })
      useAppStore.setState({ refreshToken: pair.refreshToken })
      return await invoke<T>(cmd, args)
    } catch {
      useAppStore.getState().logout()
      throw e
    }
  }
}

export type ThemeName =
  | 'abyss'
  | 'midnight'
  | 'nebula'
  | 'pine'
  | 'canvas'
  | 'dawn'
  | 'breeze'
  | 'blossom'

export type User = {
  id: number
  email: string
}

export type Transcript = {
  id: number
  content: string
  wordCount: number
  durationSeconds: number | null
  createdAt: string
}

export type DictionaryEntry = {
  id: number
  term: string
  replacement: string
  hits: number
  createdAt: string
}

export type UsageStats = {
  totalWords: number
  speakingTimeSeconds: number
  totalSessions: number
  avgPaceWpm: number
}

type AuthReadyPayload = { authenticated: boolean; userId: number | null }

type AppState = {
  user: User | null
  refreshToken: string | null
  theme: ThemeName
  /** Persisted active route so tab survives focus-loss / tray re-open */
  activeRoute: string
  /** Persisted settings tab so it survives window focus-loss / snipping tool */
  activeSettingsTab: 'general' | 'about'
  transcripts: Transcript[]
  transcriptOffset: number
  transcriptHasMore: boolean
  filterFrom: string | null
  filterTo: string | null
  filterSortAsc: boolean
  searchQuery: string
  searchResults: Transcript[]
  isSearching: boolean
  dictionary: DictionaryEntry[]
  stats: UsageStats | null
  isLoading: boolean
  error: string | null
  authChecking: boolean
  hasHotkey: boolean
  modelReady: boolean
  modelDownloading: boolean
  downloadProgress: number
  downloadError: string | null
  updateAvailable: string | null
  listenForModelEvents: () => Promise<() => void>
  init: () => Promise<void>
  listenForAuthReady: () => Promise<() => void>
  setTheme: (theme: ThemeName) => void
  setActiveRoute: (route: string) => void
  setActiveSettingsTab: (tab: 'general' | 'about') => void
  setUser: (user: User | null) => void
  login: (email: string, password: string) => Promise<void>
  register: (email: string, password: string) => Promise<void>
  logout: () => Promise<void>
  setError: (message: string | null) => void
  fetchStats: () => Promise<void>
  setFilters: (from: string | null, to: string | null, sortAsc: boolean) => Promise<void>
  loadMoreTranscripts: () => Promise<void>
  searchTranscripts: (query: string) => Promise<void>
  addTranscript: (content: string) => Promise<void>
  updateDictionary: (term: string, replacement: string) => Promise<void>
  deleteDictionaryEntry: (id: number) => Promise<void>
}

export const useAppStore = create<AppState>()(
  persist(
    (set, get) => ({
      user: null,
      refreshToken: null,
      theme: 'abyss',
      activeRoute: '/',
      activeSettingsTab: 'general',
      transcripts: [],
      transcriptOffset: 0,
      transcriptHasMore: true,
      filterFrom: null,
      filterTo: null,
      filterSortAsc: false,
      searchQuery: '',
      searchResults: [],
      isSearching: false,
      dictionary: [],
      stats: null,
      isLoading: false,
      error: null,
      authChecking: true,
      hasHotkey: false,
      modelReady: false,
      modelDownloading: false,
      downloadProgress: 0,
      downloadError: null,
      updateAvailable: null,

      listenForModelEvents: async () => {
        // Hotkeys are app-level — fetch once on mount
        try {
          const hotkeys = await invoke<string[]>('get_registered_hotkeys')
          set({ hasHotkey: hotkeys.length > 0 })
        } catch { /* ignore */ }

        try {
          const info = await invoke<{ downloaded: boolean; downloading: boolean; downloadProgress: number; downloadError: string | null }>('get_model_info')
          if (info.downloaded) {
            set({ modelReady: true, modelDownloading: false, downloadProgress: 100, downloadError: null })
          } else if (info.downloading) {
            set({ modelDownloading: true, modelReady: false, downloadProgress: info.downloadProgress, downloadError: null })
          } else if (info.downloadError) {
            set({ modelDownloading: false, modelReady: false, downloadError: info.downloadError })
          }
        } catch { /* ignore */ }

        const u1 = await listen('model-download-start', () => {
          set({ modelDownloading: true, modelReady: false, downloadProgress: 0, downloadError: null })
        })
        const u2 = await listen<number>('model-download-progress', (e) => {
          set({ downloadProgress: e.payload, modelDownloading: true })
        })
        const u3 = await listen('model-download-complete', () => {
          set({ modelReady: true, modelDownloading: false, downloadProgress: 100, downloadError: null })
        })
        const u4 = await listen<string>('model-download-error', (e) => {
          set({ modelDownloading: false, downloadError: e.payload ?? 'Download failed' })
        })
        return () => { u1(); u2(); u3(); u4() }
      },

      init: async () => {
        if (!get().user) return
        set({ isLoading: true, error: null, transcriptOffset: 0, transcriptHasMore: true, filterFrom: null, filterTo: null, filterSortAsc: false, searchQuery: '', searchResults: [] })
        try {
          const [transcripts, dictionary, hotkeys] = await Promise.all([
            invokeWithRefresh<Transcript[]>('get_transcripts', { limit: 50, offset: 0, from: null, to: null, sortAsc: false }),
            invokeWithRefresh<DictionaryEntry[]>('get_dictionary'),
            invoke<string[]>('get_registered_hotkeys'),
          ])
          invokeWithRefresh<UsageStats>('get_usage_stats')
            .then(stats => set({ stats }))
            .catch(() => {})
          set({ transcripts, transcriptOffset: transcripts.length, transcriptHasMore: transcripts.length === 50, dictionary, hasHotkey: hotkeys.length > 0 })
        } catch (e) {
          set({ error: e instanceof Error ? e.message : 'Failed to load data' })
        } finally {
          set({ isLoading: false })
        }
      },

      listenForAuthReady: async () => {
        // Register event listeners FIRST before any invoke — events are for runtime changes only
        const unlistenReady = await listen<number>('auth:ready', async (event) => {
          if (!get().authChecking) return // already resolved via poll
          set({ authChecking: false, user: { id: event.payload, email: '' } })
          try {
            const u = await invoke<{ id: number; email: string } | null>('get_current_user')
            if (u) set({ user: { id: u.id, email: u.email } })
          } catch { /* ignore */ }
          invoke('retry_model_download').catch(() => {})
          get().init()
        })
        const unlistenUnauth = await listen<AuthReadyPayload>('auth:unauthenticated', () => {
          if (!get().authChecking) return // already resolved via poll
          set({ authChecking: false, user: null, transcripts: [], dictionary: [], stats: null, error: null })
        })

        // Pull initial auth state with retry backoff — never rely on startup events
        const MAX_ATTEMPTS = 10
        const BACKOFF_MS = 300
        let resolved = false
        for (let i = 0; i < MAX_ATTEMPTS; i++) {
          try {
            const state = await invoke<{ authenticated: boolean; userId: number | null }>('get_auth_state')
            if (state.authenticated && state.userId != null) {
              set({ authChecking: false, user: { id: state.userId, email: '' } })
              invoke<{ id: number; email: string } | null>('get_current_user')
                .then(u => { if (u) set({ user: { id: u.id, email: u.email } }) })
                .catch(() => {})
              invoke('retry_model_download').catch(() => {})
              get().init()
            } else {
              set({ authChecking: false, user: null, transcripts: [], dictionary: [], stats: null, error: null })
            }
            resolved = true
            break
          } catch {
            // Backend not ready yet — wait and retry
            await new Promise(r => setTimeout(r, BACKOFF_MS))
          }
        }

        if (!resolved) {
          // All retries exhausted — unblock the UI
          set({ authChecking: false, user: null, transcripts: [], dictionary: [], stats: null, error: null })
        }

        return () => {
          unlistenReady()
          unlistenUnauth()
        }
      },

      setTheme: (theme) => set({ theme }),
      setActiveRoute: (route) => set({ activeRoute: route }),
      setActiveSettingsTab: (tab) => set({ activeSettingsTab: tab }),
      setUser: (user) => set({ user }),

      login: async (email, password) => {
        set({ error: null })
        try {
          const resp = await invoke<{ user: User; tokens: { accessToken: string; refreshToken: string; expiresInSeconds: number } }>('login_with_tokens', { email, password })
          set({ user: resp.user, refreshToken: resp.tokens.refreshToken })
          await invoke('store_refresh_token', {
            refreshToken: resp.tokens.refreshToken,
            userId: resp.user.id,
            accessToken: resp.tokens.accessToken,
          })
          invoke('retry_model_download').catch(() => {})
          get().init()
        } catch (e) {
          const message =
            typeof e === 'object' && e !== null && 'message' in e
              ? String((e as { message: unknown }).message)
              : 'Login failed'
          set({ error: message })
          throw new Error(message, { cause: e })
        }
      },

      register: async (email, password) => {
        set({ error: null })
        try {
          const resp = await invoke<{ user: User; tokens: { accessToken: string; refreshToken: string; expiresInSeconds: number } }>('register_with_tokens', { email, password })
          set({ user: resp.user, refreshToken: resp.tokens.refreshToken })
          await invoke('store_refresh_token', {
            refreshToken: resp.tokens.refreshToken,
            userId: resp.user.id,
            accessToken: resp.tokens.accessToken,
          })
          invoke('retry_model_download').catch(() => {})
          get().init()
        } catch (e) {
          const message =
            typeof e === 'object' && e !== null && 'message' in e
              ? String((e as { message: unknown }).message)
              : 'Registration failed'
          set({ error: message })
          throw new Error(message, { cause: e })
        }
      },

      logout: async () => {
        const token = get().refreshToken
        await invoke('clear_stored_token', { refreshToken: token }).catch(() => {})
        set({ user: null, refreshToken: null, error: null, transcripts: [], transcriptOffset: 0, transcriptHasMore: true, filterFrom: null, filterTo: null, filterSortAsc: false, searchQuery: '', searchResults: [], dictionary: [], stats: null })
      },

      setError: (message) => set({ error: message }),

      fetchStats: async () => {
        try {
          const stats = await invokeWithRefresh<UsageStats>('get_usage_stats')
          set({ stats })
        } catch {
          set({ stats: null })
        }
      },

      setFilters: async (from, to, sortAsc) => {
        set({ filterFrom: from, filterTo: to, filterSortAsc: sortAsc, transcriptOffset: 0, transcriptHasMore: true, transcripts: [] })
        try {
          const items = await invokeWithRefresh<Transcript[]>('get_transcripts', { limit: 50, offset: 0, from, to, sortAsc })
          set({ transcripts: items, transcriptOffset: items.length, transcriptHasMore: items.length === 50 })
        } catch { /* ignore */ }
        // Re-run search if active
        const { searchQuery } = get()
        if (searchQuery.trim()) {
          set({ isSearching: true })
          try {
            const results = await invokeWithRefresh<Transcript[]>('search_transcripts', { query: searchQuery, limit: 50, offset: 0, from, to, sortAsc })
            set({ searchResults: results })
          } catch { set({ searchResults: [] }) }
          finally { set({ isSearching: false }) }
        }
      },

      loadMoreTranscripts: async () => {
        const { transcriptOffset, transcriptHasMore, transcripts, filterFrom, filterTo, filterSortAsc } = get()
        if (!transcriptHasMore) return
        try {
          const more = await invokeWithRefresh<Transcript[]>('get_transcripts', { limit: 50, offset: transcriptOffset, from: filterFrom, to: filterTo, sortAsc: filterSortAsc })
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
          const results = await invokeWithRefresh<Transcript[]>('search_transcripts', { query, limit: 50, offset: 0, from: filterFrom, to: filterTo, sortAsc: filterSortAsc })
          set({ searchResults: results })
        } catch {
          set({ searchResults: [] })
        } finally {
          set({ isSearching: false })
        }
      },

      addTranscript: async (content) => {
        set({ error: null })
        try {
          const newTranscript = await invokeWithRefresh<Transcript>('save_transcript', { content })
          set((state) => ({ transcripts: [newTranscript, ...state.transcripts] }))
        } catch (e) {
          set({ error: e instanceof Error ? e.message : 'Failed to save transcript' })
        }
      },

      updateDictionary: async (term, replacement) => {
        set({ error: null })
        try {
          const newEntry = await invokeWithRefresh<DictionaryEntry>('update_dictionary', { term, replacement })
          set((state) => {
            const index = state.dictionary.findIndex((d) => d.term === term)
            if (index > -1) {
              const newDictionary = [...state.dictionary]
              newDictionary[index] = newEntry
              return { dictionary: newDictionary }
            }
            return { dictionary: [newEntry, ...state.dictionary] }
          })
        } catch (e) {
          set({ error: e instanceof Error ? e.message : 'Failed to update dictionary' })
        }
      },

      deleteDictionaryEntry: async (id) => {
        set({ error: null })
        try {
          await invokeWithRefresh('delete_dictionary_entry', { id })
          set((state) => ({ dictionary: state.dictionary.filter((d) => d.id !== id) }))
        } catch (e) {
          set({ error: e instanceof Error ? e.message : 'Failed to delete entry' })
        }
      },
    }),
    {
      name: 'nexus-voice-storage',
      // Persist UI prefs + last active route so tab survives tray minimize
      partialize: (state) => ({ theme: state.theme, activeRoute: state.activeRoute, activeSettingsTab: state.activeSettingsTab }),
    }
  )
)
