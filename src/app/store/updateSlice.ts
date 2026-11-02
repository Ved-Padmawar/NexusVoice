import { check } from '@tauri-apps/plugin-updater'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage, hasErrorCode } from '../../shared/errors'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'

export type UpdateStatus =
  | 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error' | 'up-to-date'

/** The pending update, shared so the sidebar card and the About tab drive one
 *  install rather than each holding a private updater handle. */
export type UpdateSlice = {
  updateStatus: UpdateStatus
  updateVersion: string | null
  updateProgress: number
  updateError: string | null
  checkForUpdate: () => Promise<void>
  downloadUpdate: () => Promise<void>
  /** Install the downloaded update and restart into it. */
  restartForUpdate: () => Promise<void>
}

export const createUpdateSlice: StateCreator<AppState, [], [], UpdateSlice> = (set, get) => ({
  updateStatus: 'idle',
  updateVersion: null,
  updateProgress: 0,
  updateError: null,

  checkForUpdate: async () => {
    set({ updateStatus: 'checking', updateError: null })
    try {
      const update = await check()
      if (update?.available) {
        updateHandle = update
        installation = null
        set({ updateStatus: 'available', updateVersion: update.version })
      } else {
        set({ updateStatus: 'up-to-date', updateVersion: null })
      }
    } catch (e) {
      set({ updateStatus: 'error', updateError: extractErrorMessage(e, 'Update check failed') })
    }
  },

  downloadUpdate: async () => {
    const update = updateHandle
    if (!update || get().updateStatus === 'downloading') return
    set({ updateStatus: 'downloading', updateProgress: 0, updateError: null })
    try {
      let total = 0
      let downloaded = 0
      await update.download((progress) => {
        if (progress.event === 'Started') {
          total = progress.data.contentLength ?? 0
        } else if (progress.event === 'Progress') {
          downloaded += progress.data.chunkLength
          if (total === 0) return
          // Chunks far outnumber percent steps, and each `set` rewrites persisted state.
          const pct = Math.round((downloaded / total) * 100)
          if (pct !== get().updateProgress) set({ updateProgress: pct })
        } else if (progress.event === 'Finished') {
          set({ updateProgress: 100 })
        }
      })
      set({ updateStatus: 'ready' })
    } catch (e) {
      set({ updateStatus: 'error', updateError: extractErrorMessage(e, 'Download failed') })
    }
  },

  restartForUpdate: async () => {
    const update = updateHandle
    if (!update) return
    try {
      // Kept, so a double click installs once.
      installation ??= install(update).catch((e: unknown) => {
        installation = null
        throw e
      })
      await installation
      await commands.relaunchApp()
    } catch (e) {
      if (hasErrorCode(e, 'recording_in_progress')) {
        // Still ready: Restart works once the dictation ends.
        toast.error(extractErrorMessage(e, 'Finish the current dictation first'))
      } else {
        set({ updateStatus: 'error', updateError: extractErrorMessage(e, 'Restart failed') })
      }
    }
  },
})

/** The plugin's updater handle. Kept outside the store because it is a live
 *  object with methods, not serializable state. */
let updateHandle: Awaited<ReturnType<typeof check>> | null = null

/** The install of `updateHandle`, once started. */
let installation: Promise<void> | null = null

/** Installing can end the process (Windows), so Rust first refuses new
 *  recordings, and lets them back in if the install fails. */
async function install(update: NonNullable<typeof updateHandle>) {
  await commands.prepareRestart()
  try {
    await update.install()
  } catch (e) {
    await commands.cancelRestart()
    throw e
  }
}
