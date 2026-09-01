import { useState, useCallback, useEffect, useMemo, lazy, Suspense } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from '../../lib/commands'
import {
  formatModelSize, isStreaming, modelNameToId, sortForDisplay,
  type CatalogModel, type ModelId,
} from '../../lib/models'
import { toast } from 'sonner'
import {
  Check, Cpu, Database, Download, Globe, HardDrive, Mic, Radio, Search, X,
} from 'lucide-react'
import { Input } from '@/components/ui/input'
import { VendorMark } from '../../components/ui/VendorMark'
import { vendorForFamily } from '../../lib/vendors'
import type { HardwareProfile } from '../../types'
import { useAppStore } from '../../store/useAppStore'
import type { Download as ModelDownload } from '../../store/modelSlice'

// Opens on demand, so its tree stays out of the Settings chunk.
const ModelManagerModal = lazy(() =>
  import('../../components/ModelManagerModal').then(m => ({ default: m.ModelManagerModal }))
)

type DownloadedModel = {
  variant: string
  displayName: string
  sizeBytes: number
  isActive: boolean
}

/** Quantised — a proportional bar made small models invisible slivers. */
function weightTier(bytes: number): 0 | 1 | 2 | 3 {
  if (bytes >= 1_000_000_000) return 3
  if (bytes >= 600_000_000) return 2
  if (bytes >= 180_000_000) return 1
  return 0
}

const TIER_LABEL = ['Light', 'Medium', 'Heavy', 'Max'] as const
const TIER_HEIGHTS = ['4px', '6px', '8px', '10px']

function WeightMeter({ bytes, active }: { bytes: number; active: boolean }) {
  const tier = weightTier(bytes)
  return (
    <span className="flex items-end gap-[2px]" title={`${TIER_LABEL[tier]} download`}>
      {[0, 1, 2, 3].map((i) => (
        <span
          key={i}
          className="w-[3px] rounded-[1px]"
          style={{
            height: TIER_HEIGHTS[i],
            background: i <= tier ? (active ? 'var(--accent)' : 'var(--fg-2)') : 'var(--border)',
          }}
        />
      ))}
    </span>
  )
}

/** Card hover backs off over a control, so only one surface lights up. */
function ModelCard({
  model,
  loaded,
  installed,
  recommended,
  download,
  disabled,
  onDownload,
  onUse,
  onCancel,
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
  const streaming = isStreaming(model)
  const vendor = vendorForFamily(model.family)

  return (
    <div
      title={model.displayName}
      className={`nv-edge flex flex-col gap-2.5 rounded-(--r-lg) p-3 ${
        loaded
          ? '[--edge:var(--accent)] bg-(--accent-soft)'
          : '[--edge:var(--border-soft)] bg-(--panel) hover:[--edge:var(--border)] hover:bg-(--surface-hover) has-[button:hover]:[--edge:var(--border-soft)] has-[button:hover]:bg-(--panel)'
      }`}
    >
      <div className="flex gap-2.5">
        {/* The vendor that trained the model — a real mark reads faster than
            an initial, and groups the families without an accordion. */}
        <span
          className={`grid size-8 shrink-0 place-items-center rounded-(--r-md) ${
            loaded
              ? 'border border-(--accent) bg-(--accent-soft)'
              : installed
                ? 'border border-(--accent-soft) bg-(--accent-soft)'
                : 'border border-(--border) bg-(--surface)'
          }`}
        >
          {vendor
            ? <VendorMark vendor={vendor} className="size-4.5" />
            : <Cpu size={15} strokeWidth={1.75} className="text-muted-foreground" />}
        </span>

        <div className="min-w-0 flex-1">
          <div className="flex items-start gap-1.5">
            <span className={`min-w-0 flex-1 truncate text-[12.5px] font-semibold tracking-[-0.01em] ${loaded ? 'text-(--accent)' : 'text-(--fg)'}`}>
              {model.displayName}
            </span>
            {recommended && !loaded && (
              <span className="shrink-0 rounded-(--r-xs) border border-(--accent-soft) bg-(--accent-soft) px-1.5 py-px text-[9px] font-semibold uppercase tracking-[0.04em] text-(--accent)">
                Best fit
              </span>
            )}
          </div>
          <p className="mt-0.5 truncate text-[11px] text-muted-foreground">{model.description}</p>
        </div>
      </div>

      <div className="flex items-center gap-2">
        <WeightMeter bytes={model.sizeBytes} active={loaded} />
        <span className={`text-[11px] font-medium tabular-nums ${loaded ? 'text-(--accent)' : 'text-(--fg-2)'}`}>
          {formatModelSize(model.sizeBytes)}
        </span>

        <span className="ml-auto flex items-center gap-1.5">
          {streaming && (
            <span
              title="Streaming — text appears while you speak"
              className="flex shrink-0 items-center gap-1 rounded-(--r-xs) bg-(--accent-soft) px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-[0.04em] text-(--accent)"
            >
              <Radio size={9} strokeWidth={2.5} />
              streaming
            </span>
          )}
          <span
            title={model.multilingual ? 'Multilingual' : 'English only'}
            className="flex shrink-0 items-center gap-1 text-[9px] font-semibold uppercase tracking-[0.04em] text-muted-foreground"
          >
            <Globe size={9} strokeWidth={2.25} />
            {model.multilingual ? 'multi' : 'en'}
          </span>
        </span>
      </div>

      {/* Fixed height, so a card does not resize as its state changes. */}
      <div className="flex h-7 items-center gap-2">
        {download && download.status !== 'error' ? (
          <>
            {download.status === 'queued' ? (
              <span className="flex-1 text-[11px] text-muted-foreground">Queued</span>
            ) : (
              <>
                <span className="h-1.5 flex-1 overflow-hidden rounded-full bg-(--surface)">
                  <motion.span
                    className="block h-full rounded-full bg-(--accent)"
                    initial={false}
                    animate={{ width: `${download.progress}%` }}
                    transition={{ duration: 0.25 }}
                  />
                </span>
                <span className="shrink-0 text-[11px] font-semibold tabular-nums text-(--accent)">
                  {download.progress}%
                </span>
              </>
            )}
            <motion.button
              type="button"
              aria-label="Cancel download"
              title="Cancel download"
              onClick={onCancel}
              className="grid size-5 shrink-0 place-items-center rounded-(--r-xs) text-muted-foreground cursor-pointer"
              whileHover={{ color: 'var(--danger)', backgroundColor: 'color-mix(in srgb, var(--danger) 12%, transparent)' }}
              whileTap={{ scale: 0.92 }}
              transition={{ duration: 0.15 }}
            >
              <X size={13} strokeWidth={2.25} />
            </motion.button>
          </>
        ) : loaded ? (
          // `leading-none`, or the icon centres against the taller line box.
          <span className="nv-edge [--edge:var(--accent)] flex h-7 w-full items-center justify-center gap-1.5 rounded-(--r-md) bg-(--accent-soft) text-[12px] font-semibold text-(--accent)">
            <Check size={12} strokeWidth={2.5} className="shrink-0" />
            <span className="leading-none">In use</span>
          </span>
        ) : (
          <button
            type="button"
            onClick={installed ? onUse : onDownload}
            disabled={disabled}
            title={download?.error ?? undefined}
            className={`nv-edge flex h-7 w-full items-center justify-center gap-1.5 rounded-(--r-md) bg-(--surface) text-[12px] font-medium cursor-pointer hover:bg-(--accent-soft) hover:text-(--accent) hover:[--edge:color-mix(in_srgb,var(--accent)_45%,transparent)] disabled:cursor-not-allowed disabled:opacity-50 ${
              download?.status === 'error'
                ? '[--edge:var(--danger)] text-(--danger)'
                : 'text-(--fg-2)'
            }`}
          >
            {installed ? (
              <span className="leading-none">Use this model</span>
            ) : (
              <>
                <Download size={12} strokeWidth={2} className="shrink-0" />
                <span className="leading-none">
                  {download?.status === 'error' ? 'Retry download' : 'Download'}
                </span>
              </>
            )}
          </button>
        )}
      </div>
    </div>
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

  const refreshOnDisk = useCallback(() => {
    invoke<DownloadedModel[]>(COMMANDS.GET_DOWNLOADED_MODELS).then(setOnDisk).catch(() => setOnDisk([]))
  }, [])

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

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase()
    return orderedModels.filter((m) => {
      if (filter === 'installed' && !onDiskVariants.has(m.id)) return false
      if (filter === 'streaming' && !isStreaming(m)) return false
      if (filter === 'multilingual' && !m.multilingual) return false
      if (q && !m.displayName.toLowerCase().includes(q) && !m.description.toLowerCase().includes(q)) {
        return false
      }
      return true
    })
  }, [orderedModels, filter, query, onDiskVariants])

  const filterCount = useCallback((id: Filter) => {
    if (id === 'all') return orderedModels.length
    return orderedModels.filter((m) =>
      id === 'installed' ? onDiskVariants.has(m.id)
        : id === 'streaming' ? isStreaming(m)
          : m.multilingual,
    ).length
  }, [orderedModels, onDiskVariants])

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
    <div className="flex flex-col gap-3">
      {/* What is running — a status readout, so one line. */}
      <div className={`flex shrink-0 items-center gap-2.5 rounded-(--r-md) border px-3 py-2 ${
        loadedVariant ? 'border-(--accent-soft) bg-(--accent-soft)' : 'border-(--border-soft) bg-(--panel)'
      }`}>
        <span className={`grid size-7 shrink-0 place-items-center rounded-(--r-sm) ${
          loadedVariant ? 'bg-(--accent-soft)' : 'text-muted-foreground'
        }`}>
          {shownVendor
            ? <VendorMark vendor={shownVendor} className="size-4" />
            : <Mic size={13} strokeWidth={2} />}
        </span>

        <span className="shrink-0 truncate text-[13px] font-bold tracking-[-0.02em] text-(--fg)">
          {shownModel ? shownModel.displayName : 'No model loaded'}
        </span>

        {shownModel && isStreaming(shownModel) && (
          <span className="flex shrink-0 items-center gap-1 rounded-(--r-xs) bg-(--accent-soft) px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-[0.04em] text-(--accent)">
            <Radio size={9} strokeWidth={2.5} />
            live
          </span>
        )}
        <span className="min-w-0 flex-1 truncate text-[10px] text-muted-foreground">
          {shownModel ? shownModel.detail : 'Pick a model below to start transcribing.'}
        </span>

        <span className="flex shrink-0 items-center gap-3 border-l border-(--border-soft) pl-3 text-[10px]">
          <span className="flex items-center gap-1.5 text-(--fg-2)">
            <Cpu size={11} className="text-(--accent)" />
            {profile
              ? `${profile.executionProvider.toUpperCase()}${profile.vramGb > 0 ? ` · ${profile.vramGb} GB` : ''}`
              : 'Detecting…'}
          </span>
          <span className="flex items-center gap-1.5 text-muted-foreground">
            <HardDrive size={11} />
            {onDisk.length} on disk · {(diskBytes / 1e9).toFixed(2)} GB
          </span>
        </span>

        <button
          type="button"
          onClick={() => setManagerOpen(true)}
          title="Manage downloaded models"
          className="flex shrink-0 items-center gap-1.5 rounded-(--r-sm) border border-(--border) bg-(--surface) px-2 py-1 text-[11px] font-medium text-(--fg-2) cursor-pointer transition-colors duration-(--t-fast) hover:bg-(--surface-hover) hover:text-(--fg)"
        >
          <Database size={11} strokeWidth={1.75} className="shrink-0" />
          <span className="leading-none">Manage</span>
        </button>
      </div>

      <div className="flex shrink-0 items-center gap-2">
        <div className="flex items-center gap-1">
          {FILTERS.map((f) => {
            const on = filter === f.id
            return (
              <button
                key={f.id}
                type="button"
                onClick={() => setFilter(f.id)}
                className={`nv-edge flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[11px] font-medium cursor-pointer ${
                  on
                    ? '[--edge:color-mix(in_srgb,var(--accent)_55%,transparent)] bg-(--accent-soft) text-(--accent)'
                    : 'text-(--fg-2) hover:[--edge:var(--muted)] hover:text-(--fg)'
                }`}
              >
                {f.label}
                <span className="tabular-nums">{filterCount(f.id)}</span>
              </button>
            )
          })}
        </div>

        <div className="relative w-42 shrink-0">
          <Search size={12} strokeWidth={2} className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Find a model…"
            aria-label="Find a model"
            className="h-7 pl-7 pr-2 text-[11px]"
          />
        </div>

        <span className="ml-auto min-w-0 truncate text-right text-[10px] text-muted-foreground">
          Bars show download weight — light to heavy.
        </span>
      </div>

      <div className="grid grid-cols-3 gap-2.5">
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
        <p className="py-8 text-center text-[12px] text-muted-foreground">
          No models match that filter.
        </p>
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
    </div>
  )
}
