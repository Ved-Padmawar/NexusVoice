import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { COMMANDS } from '../lib/commands'
import { EVENTS } from '../lib/events'
import { fetchModelCatalog, modelNameToId, type CatalogModel, type ModelId } from '../lib/models'
import type { ModelInfo } from '../types'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'

export type DownloadStatus = 'queued' | 'running' | 'error'

export type Download = {
  status: DownloadStatus
  progress: number
  error?: string | null
}

export type ModelSlice = {
  hasHotkey: boolean
  hasDictationHotkey: boolean
  hasDictationCommitHotkey: boolean
  modelReady: boolean
  /** Every download queued, running, or holding an error, keyed by model id. */
  downloads: Record<string, Download>
  /** Canonical active-model state (single source of truth for the main window). */
  activeModelName: string | null
  selectedModel: ModelId | null
  activeModelDownloaded: boolean
  /** The model catalog, served by the backend. Empty until first refresh. */
  catalog: CatalogModel[]
  setSelectedModel: (variant: ModelId) => void
  refreshCatalog: () => Promise<void>
  refreshModelInfo: () => Promise<void>
  refreshDownloads: () => Promise<void>
  startDownload: (id: ModelId) => Promise<void>
  cancelDownload: (id: ModelId) => Promise<void>
  listenForModelEvents: () => Promise<() => void>
}

export const createModelSlice: StateCreator<AppState, [], [], ModelSlice> = (set, get) => ({
  hasHotkey: false,
  hasDictationHotkey: false,
  hasDictationCommitHotkey: false,
  modelReady: false,
  downloads: {},
  activeModelName: null,
  selectedModel: null,
  activeModelDownloaded: false,
  catalog: [],

  setSelectedModel: (variant: ModelId) => set({ selectedModel: variant }),

  refreshCatalog: async () => {
    try {
      set({ catalog: await fetchModelCatalog() })
    } catch { /* ignore */ }
  },

  refreshModelInfo: async () => {
    try {
      const info = await invoke<ModelInfo>(COMMANDS.GET_MODEL_INFO)
      // The catalog resolves display names to ids; fetch it if not yet loaded.
      let catalog = get().catalog
      if (catalog.length === 0) {
        catalog = await fetchModelCatalog().catch(() => [])
      }
      set({
        activeModelName: info.modelName,
        catalog,
        // Only model-evicted clears this, so the pill survives a download.
        selectedModel: modelNameToId(info.modelName, catalog),
        activeModelDownloaded: info.downloaded,
        modelReady: info.downloaded,
      })
    } catch { /* ignore */ }
  },

  refreshDownloads: async () => {
    try {
      const active = await invoke<(Download & { id: string })[]>(COMMANDS.GET_ACTIVE_DOWNLOADS)
      set({
        downloads: Object.fromEntries(
          active.map(({ id, status, progress, error }) => [id, { status, progress, error }]),
        ),
      })
    } catch { /* ignore */ }
  },

  startDownload: async (id: ModelId) => {
    set({ downloads: { ...get().downloads, [id]: { status: 'queued', progress: 0 } } })
    try {
      await invoke(COMMANDS.START_MODEL_DOWNLOAD, { id })
    } catch {
      set(dropDownload(get(), id))
    }
  },

  cancelDownload: async (id: ModelId) => {
    try {
      await invoke<boolean>(COMMANDS.CANCEL_MODEL_DOWNLOAD, { id })
    } catch { /* ignore */ }
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
    await get().refreshDownloads()

    const setStatus = (id: string, patch: Partial<Download>) =>
      set((s) => {
        const base: Download = s.downloads[id] ?? { status: 'queued', progress: 0 }
        return { downloads: { ...s.downloads, [id]: { ...base, ...patch } } }
      })

    const u1 = await listen<DownloadEvent>(EVENTS.MODEL_DOWNLOAD_START, (e) => {
      setStatus(e.payload.id, { status: 'queued', progress: 0, error: null })
    })
    const u2 = await listen<DownloadEvent>(EVENTS.MODEL_DOWNLOAD_RUNNING, (e) => {
      setStatus(e.payload.id, { status: 'running' })
    })
    const u3 = await listen<{ id: string; pct: number }>(EVENTS.MODEL_DOWNLOAD_PROGRESS, (e) => {
      setStatus(e.payload.id, { status: 'running', progress: e.payload.pct })
    })
    const u4 = await listen<DownloadEvent>(EVENTS.MODEL_DOWNLOAD_COMPLETE, (e) => {
      set(dropDownload(get(), e.payload.id))
      void get().refreshModelInfo()
    })
    const u5 = await listen<{ id: string; error: string }>(EVENTS.MODEL_DOWNLOAD_ERROR, (e) => {
      setStatus(e.payload.id, { status: 'error', error: e.payload.error || 'Download failed' })
    })
    const u6 = await listen<DownloadEvent>(EVENTS.MODEL_DOWNLOAD_CANCELLED, (e) => {
      set(dropDownload(get(), e.payload.id))
    })
    // Deleted active model: clear selection; keep modelChosen so the picker stays closed.
    const u7 = await listen(EVENTS.MODEL_EVICTED, () => {
      set({ modelReady: false, selectedModel: null, activeModelDownloaded: false })
    })
    // Deleted the active model but another is on disk: backend switched to it.
    const u8 = await listen(EVENTS.MODEL_SWITCHED, () => { void get().refreshModelInfo() })
    return () => { u1(); u2(); u3(); u4(); u5(); u6(); u7(); u8() }
  },
})

type DownloadEvent = { id: string }

/** Removes one download from the map, leaving the rest untouched. */
function dropDownload(state: { downloads: Record<string, Download> }, id: string) {
  const rest = { ...state.downloads }
  delete rest[id]
  return { downloads: rest }
}

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
