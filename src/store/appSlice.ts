import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from '../lib/commands'
import { extractErrorMessage } from '../lib/errors'
import { logger } from '../lib/logger'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'

export type AppSlice = {
  /** True until the database is open (or has failed to open). */
  starting: boolean
  startupError: string | null
  startup: () => Promise<void>
}

export const createAppSlice: StateCreator<AppState, [], [], AppSlice> = (set, get) => ({
  starting: true,
  startupError: null,

  startup: async () => {
    try {
      await invoke(COMMANDS.WAIT_FOR_APP_READY)
    } catch (e) {
      const message = extractErrorMessage(e, 'Database unavailable')
      logger.error('startup failed', message)
      set({ starting: false, startupError: message })
      return
    }
    set({ starting: false, startupError: null })
    get().loadTranscripts()
    get().loadStats()
    get().loadDictionary()
  },
})
