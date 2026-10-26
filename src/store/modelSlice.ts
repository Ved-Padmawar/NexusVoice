import { toast } from 'sonner'
import { commands, events } from '../bindings'
import { modelNameToId, type CatalogModel, type ModelId } from '../lib/models'
import type { DownloadStatus } from '../types'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'

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
    // Static for the app's lifetime; empty means retry.
    if (get().catalog.length > 0) return
    try {
      set({ catalog: await commands.getModelCatalog() })
    } catch { /* ignore */ }
  },

  refreshModelInfo: async () => {
    try {
      const info = await commands.getModelInfo()
      // The catalog resolves display names to ids; fetch it if not yet loaded.
      let catalog = get().catalog
      if (catalog.length === 0) {
        catalog = await commands.getModelCatalog().catch(() => [])
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
      const active = await commands.getActiveDownloads()
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
      await commands.startModelDownload(id)
    } catch {
      set(dropDownload(get(), id))
    }
  },

  cancelDownload: async (id: ModelId) => {
    try {
      await commands.cancelModelDownload(id)
    } catch { /* ignore */ }
  },

  listenForModelEvents: async () => {
    try {
      const hotkeys = await commands.getHotkeys()
      set({
        hasHotkey: hotkeys.pushToTalk !== null,
        hasDictationHotkey: hotkeys.dictation !== null,
        hasDictationCommitHotkey: hotkeys.dictationCommit !== null,
      })
    } catch { /* ignore */ }

    await get().refreshModelInfo()
    await get().refreshDownloads()

    const setStatus = (id: string, patch: Partial<Download>) =>
      set((s) => {
        const base: Download = s.downloads[id] ?? { status: 'queued', progress: 0 }
        return { downloads: { ...s.downloads, [id]: { ...base, ...patch } } }
      })

    const unlisteners = await Promise.all([
      events.modelDownloadStart.listen((e) => {
        setStatus(e.payload.id, { status: 'queued', progress: 0, error: null })
      }),
      events.modelDownloadRunning.listen((e) => {
        setStatus(e.payload.id, { status: 'running' })
      }),
      events.modelDownloadProgress.listen((e) => {
        setStatus(e.payload.id, { status: 'running', progress: e.payload.pct })
      }),
      events.modelDownloadComplete.listen((e) => {
        set(dropDownload(get(), e.payload.id))
        void get().refreshModelInfo()
      }),
      events.modelDownloadError.listen((e) => {
        setStatus(e.payload.id, { status: 'error', error: e.payload.error || 'Download failed' })
      }),
      events.modelDownloadCancelled.listen((e) => {
        set(dropDownload(get(), e.payload.id))
      }),
      // Deleted active model: clear selection; keep modelChosen so the picker stays closed.
      events.modelEvicted.listen(() => {
        set({ modelReady: false, selectedModel: null, activeModelDownloaded: false })
      }),
      // Deleted the active model but another is on disk: backend switched to it.
      events.modelSwitched.listen(() => { void get().refreshModelInfo() }),
      events.languageReset.listen(() => {
        toast.info('This model does not support your dictation language. Switched to English.')
      }),
    ])
    return () => unlisteners.forEach(fn => fn())
  },
})

/** Removes one download from the map, leaving the rest untouched. */
function dropDownload(state: { downloads: Record<string, Download> }, id: string) {
  const rest = { ...state.downloads }
  delete rest[id]
  return { downloads: rest }
}
