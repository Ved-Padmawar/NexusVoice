import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { extractErrorMessage } from '../lib/errors'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'

export type UpdateStatus =
  | 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error' | 'up-to-date'

/** The pending update, shared so the sidebar prompt and the About tab drive one
 *  install rather than each holding a private updater handle. */
export type UpdateSlice = {
  updateStatus: UpdateStatus
  updateVersion: string | null
  updateProgress: number
  updateError: string | null
  /** Dismissed for this session — hides the sidebar prompt, About still shows it. */
  updateDismissed: boolean
  checkForUpdate: () => Promise<void>
  installUpdate: () => Promise<void>
  restartForUpdate: () => Promise<void>
  dismissUpdate: () => void
}

export const createUpdateSlice: StateCreator<AppState, [], [], UpdateSlice> = (set, get) => ({
  updateStatus: 'idle',
  updateVersion: null,
  updateProgress: 0,
  updateError: null,
  updateDismissed: false,

  checkForUpdate: async () => {
    set({ updateStatus: 'checking', updateError: null })
    try {
      const update = await check()
      if (update?.available) {
        updateHandle = update
        set({ updateStatus: 'available', updateVersion: update.version })
      } else {
        set({ updateStatus: 'up-to-date', updateVersion: null })
      }
    } catch (e) {
      set({ updateStatus: 'error', updateError: extractErrorMessage(e, 'Update check failed') })
    }
  },

  installUpdate: async () => {
    const update = updateHandle
    if (!update || get().updateStatus === 'downloading') return
    set({ updateStatus: 'downloading', updateProgress: 0, updateError: null })
    try {
      let total = 0
      let downloaded = 0
      await update.downloadAndInstall((progress) => {
        if (progress.event === 'Started') {
          total = progress.data.contentLength ?? 0
        } else if (progress.event === 'Progress') {
          downloaded += progress.data.chunkLength
          if (total > 0) set({ updateProgress: Math.round((downloaded / total) * 100) })
        } else if (progress.event === 'Finished') {
          set({ updateStatus: 'ready', updateProgress: 100 })
        }
      })
    } catch (e) {
      set({ updateStatus: 'error', updateError: extractErrorMessage(e, 'Download failed') })
    }
  },

  restartForUpdate: async () => {
    try {
      await relaunch()
    } catch (e) {
      set({ updateStatus: 'error', updateError: extractErrorMessage(e, 'Restart failed') })
    }
  },

  dismissUpdate: () => set({ updateDismissed: true }),
})

/** The plugin's updater handle. Kept outside the store because it is a live
 *  object with methods, not serializable state. */
let updateHandle: Awaited<ReturnType<typeof check>> | null = null
