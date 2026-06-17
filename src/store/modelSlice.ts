import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { COMMANDS } from '../lib/commands'
import { EVENTS } from '../lib/events'
import type { ModelInfo } from '../types'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'

export type ModelSlice = {
  hasHotkey: boolean
  hasDictationHotkey: boolean
  hasDictationCommitHotkey: boolean
  modelReady: boolean
  modelDownloading: boolean
  downloadProgress: number
  downloadError: string | null
  /** Active STT engine, used to label the download banner. */
  activeEngine: 'whisper' | 'parakeet'
  updateAvailable: string | null
  /** The model override that was active before the current download started — restored on cancel. */
  downloadingFromModel: string | null
  setDownloadingFromModel: (variant: string) => void
  cancelDownload: () => void
  listenForModelEvents: () => Promise<() => void>
}

export const createModelSlice: StateCreator<AppState, [], [], ModelSlice> = (set, get) => ({
  hasHotkey: false,
  hasDictationHotkey: false,
  hasDictationCommitHotkey: false,
  modelReady: false,
  modelDownloading: false,
  downloadProgress: 0,
  downloadError: null,
  activeEngine: 'whisper',
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
      const hotkeys = await invoke<unknown>(COMMANDS.GET_REGISTERED_HOTKEYS)
      const parsed = parseRegisteredHotkeys(hotkeys)
      set({
        hasHotkey: parsed.ptt.length > 0,
        hasDictationHotkey: parsed.dictation.length > 0,
        hasDictationCommitHotkey: parsed.dictationCommit.length > 0,
      })
    } catch { /* ignore */ }

    try {
      const engine = await invoke<string>(COMMANDS.GET_ACTIVE_ENGINE)
      set({ activeEngine: engine === 'parakeet' ? 'parakeet' : 'whisper' })
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
      invoke<string>(COMMANDS.GET_ACTIVE_ENGINE)
        .then(engine => set({ activeEngine: engine === 'parakeet' ? 'parakeet' : 'whisper' }))
        .catch(() => {})
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

type ParsedHotkeys = {
  ptt: string[]
  dictation: string[]
  dictationCommit: string[]
}

function asHotkeyList(value: unknown): string[] {
  if (typeof value === 'string' && value.length > 0) return [value]
  if (Array.isArray(value)) return value.filter((item): item is string => typeof item === 'string' && item.length > 0)
  return []
}

export function parseRegisteredHotkeys(value: unknown): ParsedHotkeys {
  if (Array.isArray(value)) {
    return { ptt: asHotkeyList(value), dictation: [], dictationCommit: [] }
  }

  if (!value || typeof value !== 'object') {
    return { ptt: [], dictation: [], dictationCommit: [] }
  }

  const record = value as Record<string, unknown>
  return {
    ptt: asHotkeyList(record.ptt ?? record.pushToTalk ?? record.recording ?? record.hotkey ?? record.primary),
    dictation: asHotkeyList(record.dictation ?? record.dictationHotkey ?? record.dictation_hotkey),
    dictationCommit: asHotkeyList(record.dictationCommit ?? record.dictation_commit ?? record.dictationCommitHotkey ?? record.dictation_commit_hotkey),
  }
}
