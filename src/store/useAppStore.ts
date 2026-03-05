import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export type ThemeName =
  | 'midnight'
  | 'arctic'
  | 'slate'
  | 'warm'

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

export type ModelInfo = {
  size: string
  reason: string
  executionProvider: string
}

export type HardwareTier = 'low' | 'mid' | 'high'

export const MODEL_OPTIONS = [
  { id: 'whisper-tiny',   label: 'Whisper Tiny',   tier: 'low'  as HardwareTier },
  { id: 'whisper-base',   label: 'Whisper Base',   tier: 'low'  as HardwareTier },
  { id: 'whisper-small',  label: 'Whisper Small',  tier: 'mid'  as HardwareTier },
  { id: 'whisper-medium', label: 'Whisper Medium', tier: 'mid'  as HardwareTier },
  { id: 'whisper-large',  label: 'Whisper Large',  tier: 'high' as HardwareTier },
] as const

export type ModelId = typeof MODEL_OPTIONS[number]['id']

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
  selectedModel: ModelId
  hardwareTier: HardwareTier | null
  transcripts: Transcript[]
  dictionary: DictionaryEntry[]
  modelInfo: ModelInfo | null
  stats: UsageStats | null
  isLoading: boolean
  error: string | null
  /** True while waiting for auth:ready / auth:unauthenticated on startup */
  authChecking: boolean
  init: () => Promise<void>
  /** Subscribe to auth:ready and auth:unauthenticated events from the backend */
  listenForAuthReady: () => Promise<() => void>
  setTheme: (theme: ThemeName) => void
  setSelectedModel: (model: ModelId) => void
  setUser: (user: User | null) => void
  login: (email: string, password: string) => Promise<void>
  register: (email: string, password: string) => Promise<void>
  logout: () => Promise<void>
  setError: (message: string | null) => void
  fetchModelInfo: () => Promise<void>
  fetchHardwareTier: () => Promise<void>
  fetchStats: () => Promise<void>
  addTranscript: (content: string) => Promise<void>
  updateDictionary: (term: string, replacement: string) => Promise<void>
}

export const useAppStore = create<AppState>()(
  persist(
    (set, get) => ({
      user: null,
      refreshToken: null,
      theme: 'midnight',
      selectedModel: 'whisper-base',
      hardwareTier: null,
      transcripts: [],
      dictionary: [],
      modelInfo: null,
      stats: null,
      isLoading: false,
      error: null,
      authChecking: true,
      init: async () => {
        set({ isLoading: true, error: null })
        try {
          const [transcripts, dictionary] = await Promise.all([
            invoke<Transcript[]>('get_transcripts'),
            invoke<DictionaryEntry[]>('get_dictionary'),
          ])
          set({ transcripts, dictionary })
        } catch (e) {
          set({
            error: e instanceof Error ? e.message : 'Failed to load data',
          })
        } finally {
          set({ isLoading: false })
        }
      },
      listenForAuthReady: async () => {
        // Listen for auth:ready — backend emits this after successful silent re-auth
        const unlistenReady = await listen<number>('auth:ready', (event) => {
          // event.payload is the user_id; we don't have full user object here,
          // but the session is valid — mark as checking done with a stub user
          set({ authChecking: false, user: { id: event.payload, email: '' } })
        })
        // Listen for auth:unauthenticated — no stored token or it expired
        const unlistenUnauth = await listen<AuthReadyPayload>('auth:unauthenticated', () => {
          set({ authChecking: false, user: null })
        })
        return () => {
          unlistenReady()
          unlistenUnauth()
        }
      },
      setTheme: (theme) => set({ theme }),
      setSelectedModel: (model) => set({ selectedModel: model }),
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
      fetchModelInfo: async () => {
        try {
          const modelInfo = await invoke<ModelInfo>('get_model_info')
          set({ modelInfo })
        } catch {
          set({ modelInfo: null })
        }
      },
      fetchHardwareTier: async () => {
        try {
          const profile = await invoke<{ executionProvider: string; vramGb: number }>('get_hardware_profile')
          // Map execution provider to tier: CPU=low, DirectML=mid, CUDA=high
          const ep = profile.executionProvider.toLowerCase()
          const tier: HardwareTier = ep.includes('cuda') ? 'high' : ep.includes('directml') ? 'mid' : 'low'
          set({ hardwareTier: tier })
        } catch {
          set({ hardwareTier: null })
        }
      },
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
    }),
    {
      name: 'nexus-voice-storage',
      // Only persist UI prefs — tokens and user session are managed by the backend
      partialize: (state) => ({ theme: state.theme, selectedModel: state.selectedModel }),
    }
  )
)
