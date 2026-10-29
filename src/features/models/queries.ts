import { useCallback } from 'react'
import { useQuery } from '@tanstack/react-query'
import { toast } from 'sonner'
import { commands, events } from '../../shared/bindings'
import { live, patchLive, readFresh } from '../../app/queryClient'
import { queryKeys } from '../../shared/queryKeys'
import type { DownloadStatus, ModelInfo } from '../../shared/types'
import { modelNameToId, type CatalogModel, type ModelId } from './models'

export type Download = {
  status: DownloadStatus
  progress: number
  error?: string | null
}

/** Every download queued, running, or holding an error, keyed by model id. */
type Downloads = Record<string, Download>

export const NO_MODELS: CatalogModel[] = []

const catalogOptions = live(queryKeys.catalog, () => commands.getModelCatalog())

const activeModelOptions = live(queryKeys.activeModel, () => commands.getModelInfo())

const downloadsOptions = live(queryKeys.downloads, async (): Promise<Downloads> => Object.fromEntries(
  (await commands.getActiveDownloads()).map(({ id, status, progress, error }) => [id, { status, progress, error }]),
))

export function useCatalog() {
  return useQuery(catalogOptions)
}

/** The model Rust will load, as a catalog id once the catalog is known. */
export function useActiveModel() {
  const catalog = useCatalog().data
  const select = useCallback(
    (info: ModelInfo) => ({
      id: catalog ? modelNameToId(info.modelName, catalog) : null,
      downloaded: info.downloaded,
    }),
    [catalog],
  )
  return useQuery({ ...activeModelOptions, select })
}

export function useDownload(id: ModelId | null) {
  return useQuery({ ...downloadsOptions, select: (downloads: Downloads) => (id ? downloads[id] : undefined) })
}

export function usePendingDownloads() {
  return useQuery({ ...downloadsOptions, select: (downloads: Downloads) => Object.keys(downloads).length })
}

export const refreshActiveModel = () => readFresh(activeModelOptions)

/** Make `id` the model Rust loads; resolves to the new active model. */
export async function switchModel(id: ModelId) {
  await commands.setModelOverride(id)
  return refreshActiveModel()
}

const setDownload = (id: string, patch: Partial<Download>) =>
  patchLive(downloadsOptions, (downloads) => {
    const current: Download = downloads[id] ?? { status: 'queued', progress: 0 }
    return { ...downloads, [id]: { ...current, ...patch } }
  })

const dropDownload = (id: string) =>
  patchLive(downloadsOptions, (downloads) => {
    const rest = { ...downloads }
    delete rest[id]
    return rest
  })

export async function startDownload(id: ModelId) {
  setDownload(id, { status: 'queued', progress: 0 })
  try {
    await commands.startModelDownload(id)
  } catch {
    dropDownload(id)
  }
}

export async function cancelDownload(id: ModelId) {
  try {
    await commands.cancelModelDownload(id)
  } catch { /* the cancelled event drops it */ }
}

const rereadActiveModel = () => { refreshActiveModel().catch(() => {}) }

export async function listenForModelEvents(): Promise<() => void> {
  const unlisteners = await Promise.all([
    events.modelDownloadStart.listen(e => setDownload(e.payload.id, { status: 'queued', progress: 0, error: null })),
    events.modelDownloadRunning.listen(e => setDownload(e.payload.id, { status: 'running' })),
    events.modelDownloadProgress.listen(e => setDownload(e.payload.id, { status: 'running', progress: e.payload.pct })),
    events.modelDownloadComplete.listen((e) => {
      dropDownload(e.payload.id)
      rereadActiveModel()
    }),
    events.modelDownloadError.listen(e => setDownload(e.payload.id, { status: 'error', error: e.payload.error || 'Download failed' })),
    events.modelDownloadCancelled.listen(e => dropDownload(e.payload.id)),
    // Deleting the active model unloads it, or switches to another on disk.
    events.modelEvicted.listen(rereadActiveModel),
    events.modelSwitched.listen(rereadActiveModel),
    events.languageReset.listen(() => {
      toast.info('This model does not support your dictation language. Switched to English.')
    }),
  ])
  return () => unlisteners.forEach(fn => fn())
}
