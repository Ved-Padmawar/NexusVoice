import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from '../lib/commands'
import { extractErrorMessage } from '../lib/errors'
import { logger } from '../lib/logger'
import { prefetchDictionary, prefetchStats, prefetchTranscripts } from '../lib/queries'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'

export type AppSlice = {
  /** True until the initial page data is ready, or the database has failed. */
  starting: boolean
  startupError: string | null
  startup: () => Promise<void>
}

export const createAppSlice: StateCreator<AppState, [], [], AppSlice> = (set) => {
  let startupInFlight: Promise<void> | null = null
  return {
    starting: true,
    startupError: null,

    startup: () => {
      // StrictMode can start this twice before the first IPC response arrives.
      if (startupInFlight) return startupInFlight
      startupInFlight = (async () => {
        try {
          await invoke(COMMANDS.WAIT_FOR_APP_READY)
        } catch (e) {
          const message = extractErrorMessage(e, 'Database unavailable')
          logger.error('startup failed', message)
          set({ starting: false, startupError: message })
          return
        }
        // Reveal the initial page with its data together, rather than moving the
        // feed after each response. Each query handles its own error state.
        await Promise.all([prefetchTranscripts(), prefetchStats(), prefetchDictionary()])
        set({ starting: false, startupError: null })
      })().finally(() => { startupInFlight = null })
      return startupInFlight
    },
  }
}
