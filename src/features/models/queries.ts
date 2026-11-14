import { useCallback } from 'react'
import { useQuery } from '@tanstack/react-query'
import { toast } from 'sonner'
import { commands, events } from '../../shared/bindings'
import { caughtUp, reads } from '../../app/sync'
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

// Dev and release share the model files, and neither hears the other's
// changes: what is on disk is read again whenever a view opens or the
// window comes forward.
const ON_DISK = { ...reads('models', 'settings'), refetchOnMount: 'always', refetchOnWindowFocus: 'always' } as const

export function useCatalog() {
  return useQuery({
    queryKey: queryKeys.catalog,
    queryFn: () => commands.getModelCatalog(),
    ...ON_DISK,
  })
}

/** The models on disk, with the size each takes. */
export function useDownloadedModels() {
  return useQuery({
    queryKey: queryKeys.downloadedModels,
    queryFn: () => commands.getDownloadedModels(),
    ...ON_DISK,
  })
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
  return useQuery({
    queryKey: queryKeys.activeModel,
    queryFn: () => commands.getModelInfo(),
    select,
    ...ON_DISK,
  })
}

/** The active model's languages, with the saved one marked. */
export function useLanguageOptions() {
  return useQuery({
    queryKey: queryKeys.languageOptions,
    queryFn: () => commands.getLanguageOptions(),
    ...reads('settings'),
  })
}

export function useModelUnload() {
  return useQuery({
    queryKey: queryKeys.modelUnload,
    queryFn: () => commands.getModelUnload(),
    ...reads('settings'),
  })
}

const downloadsOptions = {
  queryKey: queryKeys.downloads,
  queryFn: async (): Promise<Downloads> => Object.fromEntries(
    (await commands.getActiveDownloads()).map(({ id, status, progress, error }) => [id, { status, progress, error }]),
  ),
  ...reads('downloads'),
}

export function useDownload(id: ModelId | null) {
  return useQuery({ ...downloadsOptions, select: (downloads: Downloads) => (id ? downloads[id] : undefined) })
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

/** Make `id`, a model on disk, the one Rust loads. Settles once the screen shows it. */
export async function switchModel(id: ModelId) {
  await commands.setModelOverride(id)
  await caughtUp('settings')
}

/** Resolves false when the backend refused to start the download, else once
 * the screen shows it queued. With `activate`, Rust makes the model the
 * active one once it is on disk. */
export async function startDownload(id: ModelId, { activate = false } = {}): Promise<boolean> {
  try {
    await commands.startModelDownload(id, activate)
    await caughtUp('downloads')
    return true
  } catch {
    return false
  }
}

export async function cancelDownload(id: ModelId) {
  try {
    await commands.cancelModelDownload(id)
  } catch { /* the download list shows what Rust holds */ }
}

export function listenForLanguageReset() {
  return events.languageReset.listen(() => {
    toast.info('This model does not support your dictation language. Switched to English.')
  })
}
