import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { COMMANDS } from '../lib/commands'
import { EVENTS } from '../lib/events'
import type { ModelInfo } from '../types'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'

export type ModelSlice = {
  hasHotkey: boolean
  modelReady: boolean
  modelDownloading: boolean
  downloadProgress: number
  downloadError: string | null
  updateAvailable: string | null
  /** The model override that was active before the current download started — restored on cancel. */
  downloadingFromModel: string | null
  setDownloadingFromModel: (variant: string) => void
  cancelDownload: () => void
  listenForModelEvents: () => Promise<() => void>
}

export const createModelSlice: StateCreator<AppState, [], [], ModelSlice> = (set, get) => ({
  hasHotkey: false,
  modelReady: false,
  modelDownloading: false,
  downloadProgress: 0,
  downloadError: null,
  updateAvailable: null,
  downloadingFromModel: null,

  setDownloadingFromModel: (variant: string) => set({ downloadingFromModel: variant }),

  cancelDownload: () => {
    const prev = get().downloadingFromModel
    invoke(COMMANDS.CANCEL_MODEL_DOWNLOAD).catch(() => {})
    // Restore the Rust override to the model that was active before download
    if (prev) {
      invoke(COMMANDS.SET_MODEL_OVERRIDE, { variant: prev }).catch(() => {})
    }
  },

  listenForModelEvents: async () => {
    try {
      const hotkeys = await invoke<string[]>(COMMANDS.GET_REGISTERED_HOTKEYS)
      set({ hasHotkey: hotkeys.length > 0 })
    } catch { /* ignore */ }

    try {
      const info = await invoke<ModelInfo>(COMMANDS.GET_MODEL_INFO)
      if (info.downloaded) {
        set({ modelReady: true, modelDownloading: false, downloadProgress: 100, downloadError: null })
      } else if (info.downloading) {
        set({ modelDownloading: true, modelReady: false, downloadProgress: info.downloadProgress, downloadError: null })
      } else if (info.downloadError) {
        set({ modelDownloading: false, modelReady: false, downloadError: info.downloadError })
      }
    } catch { /* ignore */ }

    const u1 = await listen(EVENTS.MODEL_DOWNLOAD_START, () => {
      set({ modelDownloading: true, modelReady: false, downloadProgress: 0, downloadError: null })
    })
    const u2 = await listen<number>(EVENTS.MODEL_DOWNLOAD_PROGRESS, (e) => {
      set({ downloadProgress: e.payload, modelDownloading: true })
    })
    const u3 = await listen(EVENTS.MODEL_DOWNLOAD_COMPLETE, () => {
      set({ modelReady: true, modelDownloading: false, downloadProgress: 100, downloadError: null, downloadingFromModel: null })
    })
    const u4 = await listen<string>(EVENTS.MODEL_DOWNLOAD_ERROR, (e) => {
      set({ modelDownloading: false, downloadError: e.payload ?? 'Download failed', downloadingFromModel: null })
    })
    const u5 = await listen(EVENTS.MODEL_DOWNLOAD_CANCELLED, () => {
      set({ modelDownloading: false, downloadProgress: 0, downloadError: null, downloadingFromModel: null })
    })
    return () => { u1(); u2(); u3(); u4(); u5() }
  },
})
