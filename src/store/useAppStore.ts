import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export type ThemeName =
  | 'void'
  | 'obsidian'
  | 'nord'
  | 'sage'
  | 'dusk'
  | 'paper'

export type User = {
  id: number
  email: string
}

export type Transcript = {
  id: number
  content: string
  createdAt: string
}

export type DictionaryEntry = {
  id: number
  term: string
  replacement: string
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
  /** Stored in memory only — never persisted to localStorage */
  refreshToken: string | null
  theme: ThemeName
  transcripts: Transcript[]
  dictionary: DictionaryEntry[]
  stats: UsageStats | null
  isLoading: boolean
  error: string | null
  /** True while waiting for auth:ready / auth:unauthenticated on startup */
  authChecking: boolean
  hasHotkey: boolean
  /** Model download state */
  modelReady: boolean
  modelDownloading: boolean
  downloadProgress: number        // 0–100
  downloadError: string | null
  /** Update available banner */
  updateAvailable: string | null  // version string or null
  /** Subscribe to model download events from the backend */
  listenForModelEvents: () => Promise<() => void>
  init: () => Promise<void>
  /** Subscribe to auth:ready and auth:unauthenticated events from the backend */
  listenForAuthReady: () => Promise<() => void>
  setTheme: (theme: ThemeName) => void
  setUser: (user: User | null) => void
  login: (email: string, password: string) => Promise<void>
  register: (email: string, password: string) => Promise<void>
  logout: () => Promise<void>
  setError: (message: string | null) => void
  fetchStats: () => Promise<void>
  addTranscript: (content: string) => Promise<void>
  updateDictionary: (term: string, replacement: string) => Promise<void>
  deleteDictionaryEntry: (id: number) => Promise<void>
}

export const useAppStore = create<AppState>()(
  persist(
    (set, get) => ({
      user: null,
      refreshToken: null,
      theme: 'void',
      transcripts: [],
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
        // Hotkeys are app-level (not user-specific) — fetch immediately on mount
        try {
          const hotkeys = await invoke<string[]>('get_registered_hotkeys')
          set({ hasHotkey: hotkeys.length > 0 })
        } catch { /* ignore */ }

        // Poll backend for current model state (command-based — no race condition)
        const pollModelInfo = async () => {
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
        }
        await pollModelInfo()

        // Events for ongoing progress updates (supplement the initial poll)
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
        // Do not load data if not authenticated
        if (!useAppStore.getState().user) return
        set({ isLoading: true, error: null })
        try {
          const [transcripts, dictionary, hotkeys] = await Promise.all([
            invoke<Transcript[]>('get_transcripts'),
            invoke<DictionaryEntry[]>('get_dictionary'),
            invoke<string[]>('get_registered_hotkeys'),
          ])
          set({ transcripts, dictionary, hasHotkey: hotkeys.length > 0 })
        } catch (e) {
          set({ error: e instanceof Error ? e.message : 'Failed to load data' })
        } finally {
          set({ isLoading: false })
        }
      },
      listenForAuthReady: async () => {
        // Primary: poll backend auth state via command (no race condition)
        const pollAuth = async () => {
          try {
            const state = await invoke<{ authenticated: boolean; userId: number | null }>('get_auth_state')
            if (state.authenticated && state.userId != null) {
              set({ authChecking: false, user: { id: state.userId, email: '' } })
              invoke<{ id: number; email: string } | null>('get_current_user')
                .then(u => { if (u) set({ user: { id: u.id, email: u.email } }) })
                .catch(() => {})
              get().init()
              return true
            }
          } catch { /* backend not ready yet */ }
          return false
        }

        // Try immediately — backend may have finished auth already
        const resolved = await pollAuth()

        // If not resolved yet, poll again after a short delay (backend auth is async)
        let pollTimeout: ReturnType<typeof setTimeout> | null = null
        if (!resolved) {
          pollTimeout = setTimeout(async () => {
            if (!get().authChecking) return
            const ok = await pollAuth()
            if (!ok && get().authChecking) {
              // Still not authenticated — mark as unauthenticated
              set({ authChecking: false, user: null, transcripts: [], dictionary: [], stats: null, error: null })
            }
          }, 800)
        }

        // Events as backup for login/logout during the session
        const unlistenReady = await listen<number>('auth:ready', async (event) => {
          set({ authChecking: false, user: { id: event.payload, email: '' } })
          try {
            const u = await invoke<{ id: number; email: string } | null>('get_current_user')
            if (u) set({ user: { id: u.id, email: u.email } })
          } catch { /* ignore */ }
          get().init()
        })
        const unlistenUnauth = await listen<AuthReadyPayload>('auth:unauthenticated', () => {
          set({ authChecking: false, user: null, transcripts: [], dictionary: [], stats: null, error: null })
        })

        return () => {
          if (pollTimeout) clearTimeout(pollTimeout)
          unlistenReady()
          unlistenUnauth()
        }
      },
      setTheme: (theme) => set({ theme }),
      setUser: (user) => set({ user }),
      login: async (email, password) => {
        set({ error: null })
        try {
          const resp = await invoke<{ user: User; tokens: { accessToken: string; refreshToken: string; expiresInSeconds: number } }>('login_with_tokens', { email, password })
          set({ user: resp.user, refreshToken: resp.tokens.refreshToken })
          // Persist to backend secure file store
          await invoke('store_refresh_token', {
            refreshToken: resp.tokens.refreshToken,
            userId: resp.user.id,
            accessToken: resp.tokens.accessToken,
          })
        } catch (e) {
          const message =
            typeof e === 'object' && e !== null && 'message' in e
              ? String((e as { message: unknown }).message)
              : 'Login failed'
          set({ error: message })
          throw new Error(message)
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
        } catch (e) {
          const message =
            typeof e === 'object' && e !== null && 'message' in e
              ? String((e as { message: unknown }).message)
              : 'Registration failed'
          set({ error: message })
          throw new Error(message)
        }
      },
      logout: async () => {
        const token = get().refreshToken
        await invoke('clear_stored_token', { refreshToken: token }).catch(() => {})
        set({ user: null, refreshToken: null, error: null, transcripts: [], dictionary: [], stats: null })
      },
      setError: (message) => set({ error: message }),
      fetchStats: async () => {
        try {
          const stats = await invoke<UsageStats>('get_usage_stats')
          set({ stats })
        } catch {
          set({ stats: null })
        }
      },
      addTranscript: async (content) => {
        set({ error: null })
        try {
          const newTranscript = await invoke<Transcript>('save_transcript', { content })
          set((state) => ({ transcripts: [newTranscript, ...state.transcripts] }))
        } catch (e) {
          set({
            error: e instanceof Error ? e.message : 'Failed to save transcript',
          })
        }
      },
      updateDictionary: async (term, replacement) => {
        set({ error: null })
        try {
          const newEntry = await invoke<DictionaryEntry>('update_dictionary', { term, replacement })
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
          set({
            error: e instanceof Error ? e.message : 'Failed to update dictionary',
          })
        }
      },
      deleteDictionaryEntry: async (id) => {
        set({ error: null })
        try {
          await invoke('delete_dictionary_entry', { id })
          set((state) => ({ dictionary: state.dictionary.filter((d) => d.id !== id) }))
        } catch (e) {
          set({ error: e instanceof Error ? e.message : 'Failed to delete entry' })
        }
      },
    }),
    {
      name: 'nexus-voice-storage',
      // Only persist UI prefs — tokens and user session are managed by the backend
      partialize: (state) => ({ theme: state.theme }),
    }
  )
)
