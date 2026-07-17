import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { COMMANDS } from '../lib/commands'
import { EVENTS } from '../lib/events'
import { extractErrorMessage } from '../lib/errors'
import { logger } from '../lib/logger'
import { UserSchema, AuthStateSchema, type User } from '../types'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'

export type AuthSlice = {
  user: User | null
  authChecking: boolean
  listenForAuthReady: () => Promise<() => void>
  setUser: (user: User | null) => void
  login: (email: string, password: string) => Promise<void>
  register: (email: string, password: string) => Promise<void>
  logout: () => Promise<void>
}

const SIGNED_OUT_RESET: Partial<AppState> = {
  user: null,
  transcripts: [],
  dictionary: [],
  stats: null,
  transcriptsStatus: 'idle',
  transcriptsError: null,
  dictionaryStatus: 'idle',
  dictionaryError: null,
  statsStatus: 'idle',
  statsError: null,
}

function onAuthSuccess(get: () => AppState): void {
  if (get().modelChosen) invoke(COMMANDS.RETRY_MODEL_DOWNLOAD).catch(() => {})
  // Each section owns its own fetch; kick them off in parallel.
  get().loadTranscripts()
  get().loadStats()
  get().loadDictionary()
}

export const createAuthSlice: StateCreator<AppState, [], [], AuthSlice> = (set, get) => ({
  user: null,
  authChecking: true,

  listenForAuthReady: async () => {
    const unlistenReady = await listen<number>(EVENTS.AUTH_READY, async (event) => {
      if (!get().authChecking) return
      set({ authChecking: false, user: { id: event.payload, email: '' } })
      try {
        const u = await invoke<unknown>(COMMANDS.GET_CURRENT_USER)
        if (u) set({ user: UserSchema.parse(u) })
      } catch { /* ignore */ }
      onAuthSuccess(get)
    })
    const unlistenUnauth = await listen<void>(EVENTS.AUTH_UNAUTHENTICATED, () => {
      if (!get().authChecking) return
      set({ authChecking: false, ...SIGNED_OUT_RESET })
    })

    const MAX_ATTEMPTS = 10
    const BACKOFF_MS = 300
    let resolved = false
    for (let i = 0; i < MAX_ATTEMPTS; i++) {
      try {
        const authState = AuthStateSchema.parse(await invoke<unknown>(COMMANDS.GET_AUTH_STATE))
        if (authState.authenticated && authState.userId != null) {
          set({ authChecking: false, user: { id: authState.userId, email: '' } })
          invoke<unknown>(COMMANDS.GET_CURRENT_USER)
            .then(u => { if (u) set({ user: UserSchema.parse(u) }) })
            .catch(e => logger.warn('get_current_user failed', extractErrorMessage(e, String(e))))
          onAuthSuccess(get)
        } else {
          set({ authChecking: false, ...SIGNED_OUT_RESET })
        }
        resolved = true
        break
      } catch {
        await new Promise(r => setTimeout(r, BACKOFF_MS))
      }
    }
    if (!resolved) {
      set({ authChecking: false, ...SIGNED_OUT_RESET })
    }

    return () => { unlistenReady(); unlistenUnauth() }
  },

  setUser: (user) => set({ user }),

  login: async (email, password) => {
    try {
      const user = UserSchema.parse(await invoke<unknown>(COMMANDS.LOGIN, { email, password }))
      set({ user })
      onAuthSuccess(get)
    } catch (e) {
      const message = extractErrorMessage(e, 'Login failed')
      throw new Error(message, { cause: e })
    }
  },

  register: async (email, password) => {
    try {
      const user = UserSchema.parse(await invoke<unknown>(COMMANDS.REGISTER, { email, password }))
      set({ user })
      onAuthSuccess(get)
    } catch (e) {
      const message = extractErrorMessage(e, 'Registration failed')
      throw new Error(message, { cause: e })
    }
  },

  logout: async () => {
    await invoke(COMMANDS.LOGOUT).catch(() => {})
    set({
      ...SIGNED_OUT_RESET,
      transcriptHasMore: true,
      transcriptLoadingMore: false,
      filterFrom: null,
      filterTo: null,
      filterSortAsc: false,
      searchQuery: '',
      searchResults: [],
    })
  },
})
