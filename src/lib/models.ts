import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from './commands'

/** Catalog id, e.g. `whisper-medium` or `parakeet-unified-en-0.6b`. */
export type ModelId = string

/** Which decode path a model runs through. */
export type PipelineKind = 'single-shot' | 'streaming'

/** One catalog entry, as served by `get_model_catalog`. */
export type CatalogModel = {
  id: ModelId
  displayName: string
  family: string
  /** Every path this model supports; some support both. */
  pipelines: PipelineKind[]
  defaultPipeline: PipelineKind
  sizeBytes: number
  multilingual: boolean
  description: string
  detail: string
  downloaded: boolean
  isActive: boolean
}

/** The full model catalog, ascending by capability. */
export async function fetchModelCatalog(): Promise<CatalogModel[]> {
  return invoke<CatalogModel[]>(COMMANDS.GET_MODEL_CATALOG)
}

/** Human-readable download size, e.g. "886 MB". */
export function formatModelSize(bytes: number): string {
  if (bytes >= 1_000_000_000) return `${(bytes / 1_000_000_000).toFixed(1)} GB`
  return `${Math.round(bytes / 1_000_000)} MB`
}

/** Badge text, or `null` when there is nothing to call out. Every model runs
 *  locally, so only streaming is worth flagging. */
export function pipelineLabel(pipelines: PipelineKind[]): string | null {
  return pipelines.includes('streaming') ? 'Streaming' : null
}

/** Whether a model can be driven by its own streaming session. */
export function isStreaming(model: CatalogModel): boolean {
  return model.pipelines.includes('streaming')
}

/** Family display order. Unlisted families sort after these, alphabetically. */
const FAMILY_ORDER = ['whisper', 'parakeet', 'nemotron', 'qwen3-asr', 'canary', 'moonshine']

/** Group the catalog by family, smallest model first within each. */
export function sortForDisplay(catalog: CatalogModel[]): CatalogModel[] {
  const rank = (family: string) => {
    const i = FAMILY_ORDER.indexOf(family)
    return i === -1 ? FAMILY_ORDER.length : i
  }
  return [...catalog].sort((a, b) => {
    const byRank = rank(a.family) - rank(b.family)
    if (byRank !== 0) return byRank
    // Unlisted families share a rank; keep them deterministic.
    if (a.family !== b.family) return a.family.localeCompare(b.family)
    return a.sizeBytes - b.sizeBytes
  })
}


/**
 * Resolve a backend display name (e.g. from `get_model_info`) to a catalog id.
 * Falls back to the first catalog entry when nothing matches.
 */
export function modelNameToId(name: string, catalog: CatalogModel[]): ModelId | null {
  if (catalog.length === 0) return null
  const match = catalog.find(m => m.displayName.toLowerCase() === name.toLowerCase())
  return match?.id ?? catalog[0].id
}
