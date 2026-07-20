import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { COMMANDS } from '../lib/commands'
import { EVENTS } from '../lib/events'
import { modelNameToOverride, type ModelOverride } from '../lib/models'
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
  updateAvailable: string | null
  /** Canonical active-model state (single source of truth for the main window). */
  activeModelName: string | null
  selectedModel: ModelOverride | null
  activeModelDownloaded: boolean
  /** The model override that was active before the current download started — restored on cancel. */
  downloadingFromModel: string | null
  setDownloadingFromModel: (variant: string) => void
  setSelectedModel: (variant: ModelOverride) => void
  refreshModelInfo: () => Promise<void>
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
  updateAvailable: null,
  activeModelName: null,
  selectedModel: null,
  activeModelDownloaded: false,
  downloadingFromModel: null,

  setDownloadingFromModel: (variant: string) => set({ downloadingFromModel: variant }),

  setSelectedModel: (variant: ModelOverride) => set({ selectedModel: variant }),

  refreshModelInfo: async () => {
    try {
      const info = await invoke<ModelInfo>(COMMANDS.GET_MODEL_INFO)
      set({
        activeModelName: info.modelName,
        selectedModel: info.downloaded ? modelNameToOverride(info.modelName) : null,
        activeModelDownloaded: info.downloaded,
        modelReady: info.downloaded,
        modelDownloading: info.downloading,
        downloadProgress: info.downloaded ? 100 : info.downloadProgress,
        downloadError: info.downloadError ?? null,
      })
    } catch { /* ignore */ }
  },

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

    await get().refreshModelInfo()

    const u1 = await listen(EVENTS.MODEL_DOWNLOAD_START, () => {
      set({ modelDownloading: true, modelReady: false, downloadProgress: 0, downloadError: null })
    })
    const u2 = await listen<number>(EVENTS.MODEL_DOWNLOAD_PROGRESS, (e) => {
      set({ downloadProgress: e.payload, modelDownloading: true })
    })
    const u3 = await listen(EVENTS.MODEL_DOWNLOAD_COMPLETE, () => {
      set({ downloadingFromModel: null })
      void get().refreshModelInfo()
    })
    const u4 = await listen<string>(EVENTS.MODEL_DOWNLOAD_ERROR, (e) => {
      set({ modelDownloading: false, downloadError: e.payload ?? 'Download failed', downloadingFromModel: null })
    })
    const u5 = await listen(EVENTS.MODEL_DOWNLOAD_CANCELLED, () => {
      set({ modelDownloading: false, downloadProgress: 0, downloadError: null, downloadingFromModel: null })
    })
    // Deleted active model: clear selection; keep modelChosen so the picker stays closed.
    const u6 = await listen(EVENTS.MODEL_EVICTED, () => {
      set({ modelReady: false, modelDownloading: false, downloadProgress: 0, downloadError: null, selectedModel: null, activeModelDownloaded: false })
    })
    // Deleted the active model but another is on disk: backend switched to it.
    const u7 = await listen(EVENTS.MODEL_SWITCHED, () => { void get().refreshModelInfo() })
    return () => { u1(); u2(); u3(); u4(); u5(); u6(); u7() }
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
