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

/** Support is per model, so every model change re-reads it. */
export const languageOptions = live(queryKeys.languageOptions, () => commands.getLanguageOptions())

export const modelUnloadOptions = live(queryKeys.modelUnload, () => commands.getModelUnload())

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

export function useLanguageOptions() {
  return useQuery(languageOptions)
}

export function useModelUnload() {
  return useQuery(modelUnloadOptions)
}

export function useDownload(id: ModelId | null) {
  return useQuery({ ...downloadsOptions, select: (downloads: Downloads) => (id ? downloads[id] : undefined) })
}

export function usePendingDownloads() {
  return useQuery({ ...downloadsOptions, select: (downloads: Downloads) => Object.keys(downloads).length })
}

/** The furthest-behind pending download's progress, or null when none is.
 * A finished download never lowers it; a newly started one does. */
function summarizeDownloads(downloads: Downloads): number | null {
  const pending = Object.values(downloads).filter(d => d.status !== 'error')
  if (pending.length === 0) return null
  return Math.min(...pending.map(d => d.progress))
}

/** One figure for every download in flight, so concurrent ones don't take turns. */
export function useDownloadSummary() {
  return useQuery({ ...downloadsOptions, select: summarizeDownloads })
}

export const refreshActiveModel = () => readFresh(activeModelOptions)

/** The active model changed: re-read it and the languages it offers. */
const rereadModel = () => Promise.allSettled([refreshActiveModel(), readFresh(languageOptions)])

/** Make `id`, a model on disk, the one Rust loads. */
export async function switchModel(id: ModelId) {
  await commands.setModelOverride(id)
  await rereadModel()
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

/** Resolves false when the backend refused to start the download. With
 * `activate`, Rust makes the model the active one once it is on disk. */
export async function startDownload(id: ModelId, { activate = false } = {}): Promise<boolean> {
  setDownload(id, { status: 'queued', progress: 0 })
  try {
    await commands.startModelDownload(id, activate)
    return true
  } catch {
    dropDownload(id)
    return false
  }
}

export async function cancelDownload(id: ModelId) {
  try {
    await commands.cancelModelDownload(id)
  } catch { /* the cancelled event drops it */ }
}

/** Keep the download registry current. Both windows follow it, keyed by model id. */
export async function listenForDownloadEvents(): Promise<() => void> {
  const unlisteners = await Promise.all([
    events.modelDownloadStart.listen(e => setDownload(e.payload.id, { status: 'queued', progress: 0, error: null })),
    events.modelDownloadRunning.listen(e => setDownload(e.payload.id, { status: 'running' })),
    events.modelDownloadProgress.listen(e => setDownload(e.payload.id, { status: 'running', progress: e.payload.pct })),
    events.modelDownloadComplete.listen(e => dropDownload(e.payload.id)),
    events.modelDownloadError.listen(e => setDownload(e.payload.id, { status: 'error', error: e.payload.error || 'Download failed' })),
    events.modelDownloadCancelled.listen(e => dropDownload(e.payload.id)),
  ])
  return () => unlisteners.forEach(fn => fn())
}

export async function listenForModelEvents(): Promise<() => void> {
  const unlisteners = await Promise.all([
    listenForDownloadEvents(),
    // A finished download can be the active model's file.
    events.modelDownloadComplete.listen(() => void rereadModel()),
    // Deleting the active model unloads it, or switches to another on disk.
    events.modelEvicted.listen(() => void rereadModel()),
    // Also a download that asked to become the active model once on disk.
    events.modelSwitched.listen(() => void rereadModel()),
    events.languageReset.listen(() => {
      readFresh(languageOptions).catch(() => {})
      toast.info('This model does not support your dictation language. Switched to English.')
    }),
  ])
  return () => unlisteners.forEach(fn => fn())
}
