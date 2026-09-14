import { useState, useCallback, useEffect, useMemo, lazy, Suspense } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { Check, Cpu, Database, Download, HardDrive, Mic, Radio, X } from 'lucide-react'
import { COMMANDS } from '../../lib/commands'
import {
  fetchDownloadedModels, formatModelSize, isStreaming, modelNameToId, sortForDisplay,
  type CatalogModel, type ModelId,
} from '../../lib/models'
import { vendorForFamily } from '../../lib/vendors'
import { useAppStore } from '../../store/useAppStore'
import type { Download as ModelDownload } from '../../store/modelSlice'
import type { DownloadedModel, HardwareProfile } from '../../types'
import { Button, IconButton } from '@/components/ui/button'
import { SearchInput } from '@/components/ui/input'
import { Segmented } from '@/components/ui/segmented'
import { VendorMark } from '../../components/ui/VendorMark'

// Opens on demand, so its tree stays out of the Settings chunk.
const ModelManagerModal = lazy(() =>
  import('../../components/ModelManagerModal').then(m => ({ default: m.ModelManagerModal }))
)

/** Quantised — a proportional bar made small models invisible slivers. */
function weightTier(bytes: number): 0 | 1 | 2 | 3 {
  if (bytes >= 1_000_000_000) return 3
  if (bytes >= 600_000_000) return 2
  if (bytes >= 180_000_000) return 1
  return 0
}

const TIER_LABEL = ['Light', 'Medium', 'Heavy', 'Max'] as const
const TIER_HEIGHTS = [4, 6, 8, 11]

function WeightMeter({ bytes }: { bytes: number }) {
  const tier = weightTier(bytes)
  return (
    <span className="nv-meter" title={`${TIER_LABEL[tier]} download`} aria-label={`${TIER_LABEL[tier]} download`}>
      {TIER_HEIGHTS.map((h, i) => (
        <i key={i} data-on={i <= tier || undefined} style={{ height: h }} />
      ))}
    </span>
  )
}

function ModelCard({
  model, loaded, installed, recommended, download, disabled, onDownload, onUse, onCancel,
}: {
  model: CatalogModel
  loaded: boolean
  installed: boolean
  recommended: boolean
  download?: ModelDownload
  disabled: boolean
  onDownload: () => void
  onUse: () => void
  onCancel: () => void
}) {
  const vendor = vendorForFamily(model.family)

  return (
    <article className="nv-card nv-model" data-active={loaded || undefined} title={model.detail}>
      <header className="nv-model__head">
        <span className={`nv-mark ${loaded ? 'nv-mark--accent' : ''}`}>
          {vendor ? <VendorMark vendor={vendor} className="size-5" /> : <Cpu size={16} strokeWidth={1.8} />}
        </span>
        <h3 className="nv-model__name">{model.displayName}</h3>
        {recommended && !loaded && <span className="nv-badge shrink-0">Best fit</span>}
      </header>

      <p className="nv-model__desc">{model.description}</p>

      <dl className="nv-model__specs">
        <div>
          <dt>Size</dt>
          <dd><WeightMeter bytes={model.sizeBytes} />{formatModelSize(model.sizeBytes)}</dd>
        </div>
        <div>
          <dt>Language</dt>
          <dd>{model.multilingual ? 'Multilingual' : 'English'}</dd>
        </div>
        <div>
          <dt>Mode</dt>
          <dd title={isStreaming(model) ? 'Text appears while you speak' : undefined}>
            {isStreaming(model) ? 'Streaming' : 'Standard'}
          </dd>
        </div>
      </dl>

      {/* Fixed height, so the card never resizes. */}
      <div className="nv-model__foot">
        {download && download.status !== 'error' ? (
          <>
            {download.status === 'queued' ? (
              <span className="flex-1 text-[12px] text-muted">Queued</span>
            ) : (
              <>
                <span className="nv-progress">
                  <motion.span
                    className="nv-progress__fill"
                    initial={false}
                    animate={{ width: `${download.progress}%` }}
                    transition={{ duration: 0.25 }}
                  />
                </span>
                <span className="shrink-0 text-[12px] font-semibold tabular-nums text-accent-text">{download.progress}%</span>
              </>
            )}
            <IconButton label="Cancel download" tone="danger" size="sm" onClick={onCancel}>
              <X strokeWidth={2.2} />
            </IconButton>
          </>
        ) : loaded ? (
          <span className="nv-btn nv-btn--block pointer-events-none bg-(--accent-soft) text-accent-text">
            <Check strokeWidth={2.5} />In use
          </span>
        ) : (
          <Button
            variant={download?.status === 'error' ? 'danger' : installed ? 'primary' : 'secondary'}
            className="nv-btn--block"
            onClick={installed ? onUse : onDownload}
            disabled={disabled}
            title={download?.error ?? undefined}
          >
            {installed ? 'Use this model' : <><Download />{download?.status === 'error' ? 'Retry download' : 'Download'}</>}
          </Button>
        )}
      </div>
    </article>
  )
}

type Filter = 'all' | 'installed' | 'streaming' | 'multilingual'

const FILTERS: { id: Filter; label: string }[] = [
  { id: 'all', label: 'All' },
  { id: 'installed', label: 'On disk' },
  { id: 'streaming', label: 'Streaming' },
  { id: 'multilingual', label: 'Multilingual' },
]

/** The model catalog. Filters narrow the grid rather than folding it. */
export function VoiceTab() {
  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [modelSaving, setModelSaving] = useState(false)
  const [onDisk, setOnDisk] = useState<DownloadedModel[]>([])
  const [managerOpen, setManagerOpen] = useState(false)
  const [filter, setFilter] = useState<Filter>('all')
  const [query, setQuery] = useState('')

  const catalog = useAppStore(s => s.catalog)
  const orderedModels = useMemo(() => sortForDisplay(catalog), [catalog])
  const refreshCatalog = useAppStore(s => s.refreshCatalog)
  const downloads = useAppStore(s => s.downloads)
  const selected = useAppStore(s => s.selectedModel)
  const setSelectedModel = useAppStore(s => s.setSelectedModel)
  const refreshModelInfo = useAppStore(s => s.refreshModelInfo)
  const refreshDownloads = useAppStore(s => s.refreshDownloads)
  const startDownload = useAppStore(s => s.startDownload)
  const cancelDownload = useAppStore(s => s.cancelDownload)

  const refreshOnDisk = useCallback(() => { void fetchDownloadedModels().then(setOnDisk) }, [])

  useEffect(() => {
    invoke<HardwareProfile>(COMMANDS.GET_HARDWARE_PROFILE).then(setProfile).catch(() => {})
    void refreshModelInfo()
    void refreshCatalog()
    void refreshDownloads()
    refreshOnDisk()
  }, [refreshModelInfo, refreshCatalog, refreshDownloads, refreshOnDisk])

  // A finished download adds a file, so re-read the disk when the set changes.
  const pendingCount = Object.keys(downloads).length
  useEffect(() => { refreshOnDisk() }, [pendingCount, refreshOnDisk])

  const onDiskVariants = useMemo(() => new Set(onDisk.map(m => m.variant)), [onDisk])
  const recommendedVariant = profile ? modelNameToId(profile.recommendedModel, catalog) : null
  // `selected` is set even with nothing downloaded, so require the file too.
  const loadedVariant = selected && onDiskVariants.has(selected) ? selected : null
  const diskBytes = onDisk.reduce((a, m) => a + m.sizeBytes, 0)

  const matchesFilter = useCallback((m: CatalogModel, id: Filter) =>
    id === 'all' ? true
      : id === 'installed' ? onDiskVariants.has(m.id)
        : id === 'streaming' ? isStreaming(m)
          : m.multilingual,
  [onDiskVariants])

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase()
    return orderedModels.filter((m) =>
      matchesFilter(m, filter) &&
      (!q || m.displayName.toLowerCase().includes(q) || m.description.toLowerCase().includes(q)),
    )
  }, [orderedModels, filter, query, matchesFilter])

  // Only offered for a model on disk, so the override can't point at nothing.
  const handleUseModel = async (v: ModelId) => {
    setSelectedModel(v)
    setModelSaving(true)
    try {
      await invoke(COMMANDS.SET_MODEL_OVERRIDE, { variant: v })
      await refreshModelInfo()
      toast.success('Model updated')
    } catch { /* ignore */ }
    finally { setModelSaving(false) }
  }

  const shownModel = loadedVariant ? catalog.find(m => m.id === loadedVariant) : null
  const shownVendor = shownModel ? vendorForFamily(shownModel.family) : null

  return (
    <>
      <section className="nv-card nv-now" aria-label="Current model">
        <span className={`nv-mark nv-mark--lg ${shownModel ? 'nv-mark--accent' : ''}`}>
          {shownVendor ? <VendorMark vendor={shownVendor} className="size-5.5" /> : <Mic size={18} strokeWidth={1.9} />}
        </span>
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="nv-now__name">{shownModel ? shownModel.displayName : 'No model loaded'}</h2>
            {shownModel && isStreaming(shownModel) && <span className="nv-badge"><Radio />Live</span>}
          </div>
          <p className="nv-now__detail">{shownModel ? shownModel.detail : 'Pick a model below to start transcribing.'}</p>
        </div>
        <div className="nv-now__specs">
          <div className="nv-spec">
            <span className="nv-spec__label"><Cpu />Compute</span>
            {profile
              ? `${profile.executionProvider.toUpperCase()}${profile.vramGb > 0 ? `, ${profile.vramGb} GB` : ''}`
              : 'Detecting…'}
          </div>
          <div className="nv-spec">
            <span className="nv-spec__label"><HardDrive />On disk</span>
            {onDisk.length} {onDisk.length === 1 ? 'model' : 'models'}, {formatModelSize(diskBytes)}
          </div>
          <Button variant="secondary" size="sm" onClick={() => setManagerOpen(true)}>
            <Database />
            Manage
          </Button>
        </div>
      </section>

      <div className="nv-toolbar">
        <Segmented
          label="Filter models"
          value={filter}
          onChange={setFilter}
          options={FILTERS.map(f => ({
            value: f.id,
            label: f.label,
            count: orderedModels.filter(m => matchesFilter(m, f.id)).length,
          }))}
        />
        <SearchInput
          className="w-52"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Find a model…"
          aria-label="Find a model"
        />
        <span className="ml-auto flex items-center gap-2 text-[12px] text-muted">
          <span className="nv-meter" aria-hidden>
            {TIER_HEIGHTS.map((h, i) => <i key={i} data-on style={{ height: h }} />)}
          </span>
          Download size, light to heavy
        </span>
      </div>

      <div className="nv-model-grid">
        {visible.map((model) => (
          <ModelCard
            key={model.id}
            model={model}
            loaded={loadedVariant === model.id}
            installed={onDiskVariants.has(model.id)}
            recommended={recommendedVariant === model.id}
            download={downloads[model.id]}
            disabled={modelSaving}
            onDownload={() => void startDownload(model.id)}
            onUse={() => void handleUseModel(model.id)}
            onCancel={() => void cancelDownload(model.id)}
          />
        ))}
      </div>

      {visible.length === 0 && (
        <p className="py-10 text-center text-[13px] text-muted">No models match that filter.</p>
      )}

      <AnimatePresence>
        {managerOpen && (
          <Suspense fallback={null}>
            <ModelManagerModal
              onClose={() => {
                setManagerOpen(false)
                refreshOnDisk()
                void refreshModelInfo()
              }}
            />
          </Suspense>
        )}
      </AnimatePresence>
    </>
  )
}
