import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { COMMANDS } from '../lib/commands'
import { EVENTS } from '../lib/events'
import { extractErrorMessage } from '../lib/errors'
import { UserSchema, AuthResponseSchema, AuthStateSchema, type User } from '../types'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'

export type AuthSlice = {
  user: User | null
  refreshToken: string | null
  authChecking: boolean
  listenForAuthReady: () => Promise<() => void>
  setUser: (user: User | null) => void
  login: (email: string, password: string, rememberMe: boolean) => Promise<void>
  register: (email: string, password: string, rememberMe: boolean) => Promise<void>
  logout: () => Promise<void>
}

function onAuthSuccess(get: () => AppState): void {
  if (get().modelChosen) invoke(COMMANDS.RETRY_MODEL_DOWNLOAD).catch(() => {})
  get().init()
}

export const createAuthSlice: StateCreator<AppState, [], [], AuthSlice> = (set, get) => ({
  user: null,
  refreshToken: null,
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
      set({ authChecking: false, user: null, transcripts: [], dictionary: [], stats: null })
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
            .catch(e => console.warn('[store] get_current_user failed:', e))
          onAuthSuccess(get)
        } else {
          set({ authChecking: false, user: null, transcripts: [], dictionary: [], stats: null })
        }
        resolved = true
        break
      } catch {
        await new Promise(r => setTimeout(r, BACKOFF_MS))
      }
    }
    if (!resolved) {
      set({ authChecking: false, user: null, transcripts: [], dictionary: [], stats: null })
    }

    return () => { unlistenReady(); unlistenUnauth() }
  },

  setUser: (user) => set({ user }),

  login: async (email, password, rememberMe) => {
    try {
      const resp = AuthResponseSchema.parse(await invoke<unknown>(COMMANDS.LOGIN_WITH_TOKENS, { email, password }))
      set({ user: resp.user, refreshToken: rememberMe ? resp.tokens.refreshToken : null })
      if (rememberMe) {
        await invoke(COMMANDS.STORE_REFRESH_TOKEN, {
          refreshToken: resp.tokens.refreshToken,
          userId: resp.user.id,
          accessToken: resp.tokens.accessToken,
        })
      }
      onAuthSuccess(get)
    } catch (e) {
      const message = extractErrorMessage(e, 'Login failed')
      throw new Error(message, { cause: e })
    }
  },

  register: async (email, password, rememberMe) => {
    try {
      const resp = AuthResponseSchema.parse(await invoke<unknown>(COMMANDS.REGISTER_WITH_TOKENS, { email, password }))
      set({ user: resp.user, refreshToken: rememberMe ? resp.tokens.refreshToken : null })
      if (rememberMe) {
        await invoke(COMMANDS.STORE_REFRESH_TOKEN, {
          refreshToken: resp.tokens.refreshToken,
          userId: resp.user.id,
          accessToken: resp.tokens.accessToken,
        })
      }
      onAuthSuccess(get)
    } catch (e) {
      const message = extractErrorMessage(e, 'Registration failed')
      throw new Error(message, { cause: e })
    }
  },

  logout: async () => {
    const token = get().refreshToken
    await invoke(COMMANDS.CLEAR_STORED_TOKEN, { refreshToken: token }).catch(() => {})
    set({ user: null, refreshToken: null, transcripts: [], transcriptOffset: 0, transcriptHasMore: true, filterFrom: null, filterTo: null, filterSortAsc: false, searchQuery: '', searchResults: [], dictionary: [], stats: null })
  },
})
