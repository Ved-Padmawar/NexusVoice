import { useEffect } from 'react'
import { useQuery } from '@tanstack/react-query'
import { toast } from 'sonner'
import { commands, events, type Download } from '../../shared/bindings'
import { mirrored, useSlice } from '../../app/mirror'
import { queryKeys } from '../../shared/queryKeys'
import type { CatalogModel, ModelId } from './models'

export const NO_MODELS: CatalogModel[] = []

/** Every model the app offers; it never changes while the app runs. */
export function useCatalog() {
  return useQuery({
    queryKey: queryKeys.catalog,
    queryFn: () => commands.getModelCatalog(),
    staleTime: Infinity,
  })
}

/** GPU, memory, backend and the recommended model. The backend can change when
 *  a model loads, so each view that shows it asks again as it opens. */
export function useHardwareProfile() {
  return useQuery({
    queryKey: queryKeys.hardwareProfile,
    queryFn: () => commands.getHardwareProfile(),
    refetchOnMount: 'always',
  })
}

/** The models on disk, as Rust last saw them. */
export function useModelsOnDisk() {
  return useSlice('models', models => models)
}

/** Dev and release share the model files and neither hears the other, so a
 *  list of them asks Rust to look again as it opens. */
export const rescanModels = () => { commands.rescanModels().catch(() => {}) }

/** The models on disk, looked at again as the view opens. */
export function useDownloadedModels() {
  useEffect(rescanModels, [])
  return useModelsOnDisk()
}

/** Catalog id of the model recordings use. */
export function useActiveModelId() {
  return useSlice('settings', settings => settings.activeModel)
}

/** The active model's name; `undefined` while it isn't on disk. */
export function useActiveModelName() {
  const id = useActiveModelId()
  return useSlice('models', models => models.find(model => model.variant === id)?.displayName)
}

export function useActiveModelDownloaded() {
  const id = useActiveModelId()
  return useSlice('models', models => models.some(model => model.variant === id))
}

export function useDownload(id: ModelId | null): Download | undefined {
  return useSlice('downloads', downloads => downloads.find(download => download.id === id))
}

/** The furthest-behind pending download's progress, or null when none is.
 * A finished download never lowers it; a newly started one does. */
function summarizeDownloads(downloads: Download[]): number | null {
  const pending = downloads.filter(d => d.status !== 'error')
  if (pending.length === 0) return null
  return Math.min(...pending.map(d => d.progress))
}

/** One figure for every download in flight, so concurrent ones don't take turns. */
export function useDownloadSummary() {
  return useSlice('downloads', summarizeDownloads)
}

/** Make `id`, a model on disk, the one Rust loads. Settles once the screen shows it. */
export async function switchModel(id: ModelId) {
  const { rev } = await commands.setModelOverride(id)
  await mirrored(rev)
}

/** Resolves false when the backend refused to start the download, else once
 * the screen shows it queued. With `activate`, Rust makes the model the
 * active one once it is on disk. */
export async function startDownload(id: ModelId, { activate = false } = {}): Promise<boolean> {
  try {
    const { rev } = await commands.startModelDownload(id, activate)
    await mirrored(rev)
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
