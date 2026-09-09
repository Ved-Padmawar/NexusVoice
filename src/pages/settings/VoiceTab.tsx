import { useState, useCallback, useEffect, useMemo, lazy, Suspense } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Popover } from 'radix-ui'
import { COMMANDS } from '../../lib/commands'
import {
  formatModelSize, isStreaming, modelNameToId, sortForDisplay,
  type CatalogModel, type ModelId,
} from '../../lib/models'
import { toast } from 'sonner'
import { Check, Cpu, Database, Download, Globe, HardDrive, Info, Mic, Radio, Search, X } from 'lucide-react'
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
    <span className="flex items-end gap-0.5" title={`${TIER_LABEL[tier]} download`}>
      {[0, 1, 2, 3].map((i) => (
        <span
          key={i}
          className="w-0.75 rounded-[1px]"
          style={{
            height: TIER_HEIGHTS[i],
            background: i <= tier ? (active ? 'var(--accent)' : 'var(--muted)') : 'var(--border)',
          }}
        />
      ))}
    </span>
  )
}

/** Small capability marker. Reads as data, not decoration. */
function Tag({ children, on }: { children: React.ReactNode; on?: boolean }) {
  return (
    <span
      className={`flex shrink-0 items-center gap-1 rounded-(--r-xs) px-1.5 py-0.5 text-[10px] font-medium ${
        on ? 'bg-(--accent-soft) text-(--on-soft)' : 'text-(--muted)'
      }`}
    >
      {children}
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
  const streaming = isStreaming(model)
  const vendor = vendorForFamily(model.family)
  const [showDetail, setShowDetail] = useState(false)

  return (
    <div
      data-on={loaded || undefined}
      className={`flex flex-col gap-2.5 rounded-(--r-lg) p-3 transition-[background,box-shadow] duration-(--t-fast) ${
        loaded
          ? 'bg-(--accent-soft) shadow-[inset_0_0_0_1px_var(--accent-line)]'
          : 'bg-(--surface) shadow-[inset_0_0_0_1px_var(--hairline)] hover:shadow-[inset_0_0_0_1px_var(--border)]'
      }`}
    >
      <div className="flex gap-2.5">
        {/* The vendor that trained the model — a real mark reads faster than
            an initial, and groups the families without an accordion. */}
        <span className="grid size-8 shrink-0 place-items-center rounded-(--r-md) bg-(--panel) shadow-[inset_0_0_0_1px_var(--hairline)]">
          {vendor
            ? <VendorMark vendor={vendor} className="size-4.5" />
            : <Cpu size={15} strokeWidth={1.75} className="text-(--muted)" />}
        </span>

        <div className="min-w-0 flex-1">
          <div className="flex items-start gap-1.5">
            <span className={`min-w-0 flex-1 truncate text-[12.5px] font-semibold tracking-[-0.01em] ${loaded ? 'text-(--on-soft)' : 'text-(--fg)'}`}>
              {model.displayName}
            </span>
            {recommended && !loaded && (
              <span className="shrink-0 rounded-(--r-xs) bg-(--accent-soft) px-1.5 py-px text-[9.5px] font-semibold text-(--on-soft)">
                Best fit
              </span>
            )}
            {/* A real popover, not the native tooltip: it is styled, instant,
                anchored to the button rather than the whole card, and it
                floats so the grid never reflows. */}
            <Popover.Root open={showDetail} onOpenChange={setShowDetail}>
              <Popover.Trigger asChild>
                <button
                  type="button"
                  aria-label={`About ${model.displayName}`}
                  className={`iconbtn size-5 shrink-0 ${showDetail ? 'bg-(--accent-soft) text-(--on-soft)' : ''}`}
                >
                  <Info size={12} strokeWidth={2} />
                </button>
              </Popover.Trigger>
              <Popover.Portal>
                <Popover.Content
                  side="top"
                  align="end"
                  sideOffset={6}
                  collisionPadding={12}
                  className="pop z-50 w-60 rounded-(--r-md) p-3
                             data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95
                             data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95"
                >
                  <div className="flex items-center gap-2">
                    <span className="grid size-6 shrink-0 place-items-center rounded-(--r-sm) bg-(--surface) shadow-[inset_0_0_0_1px_var(--hairline)]">
                      {vendor
                        ? <VendorMark vendor={vendor} className="size-3.5" />
                        : <Cpu size={12} strokeWidth={1.75} className="text-(--muted)" />}
                    </span>
                    <p className="m-0 min-w-0 flex-1 truncate text-[11.5px] font-semibold text-(--fg)">
                      {model.displayName}
                    </p>
                  </div>
                  <p className="m-0 mt-2 text-[11.5px] leading-[1.5] text-(--muted)">{model.detail}</p>
                </Popover.Content>
              </Popover.Portal>
            </Popover.Root>
          </div>
          <p className="mt-0.5 truncate text-[11px] text-(--muted)">{model.description}</p>
        </div>
      </div>

      <div className="flex items-center gap-2">
        <WeightMeter bytes={model.sizeBytes} active={loaded} />
        <span className={`text-[11px] font-medium tabular-nums ${loaded ? 'text-(--on-soft)' : 'text-(--fg-2)'}`}>
          {formatModelSize(model.sizeBytes)}
        </span>

        <span className="ml-auto flex items-center gap-1">
          {streaming && (
            <Tag on>
              <Radio size={9} strokeWidth={2.5} />
              streaming
            </Tag>
          )}
          <Tag>
            <Globe size={9} strokeWidth={2.25} />
            {model.multilingual ? 'multilingual' : 'English'}
          </Tag>
        </span>
      </div>

      {/* Fixed height, so a card does not resize as its state changes. */}
      <div className="flex h-7 items-center gap-2">
        {download && download.status !== 'error' ? (
          <>
            {download.status === 'queued' ? (
              <span className="flex-1 text-[11px] text-(--muted)">Queued</span>
            ) : (
              <>
                <span className="h-1.5 flex-1 overflow-hidden rounded-full bg-(--bg-alt)">
                  <span
                    className="block h-full rounded-full bg-(--accent) transition-[width] duration-300 ease-out"
                    style={{ width: `${download.progress}%` }}
                  />
                </span>
                <span className="shrink-0 text-[11px] font-semibold tabular-nums text-(--on-soft)">
                  {download.progress}%
                </span>
              </>
            )}
            <button
              type="button"
              aria-label="Cancel download"
              title="Cancel download"
              onClick={onCancel}
              className="iconbtn iconbtn-danger size-5"
            >
              <X size={12} strokeWidth={2.25} />
            </button>
          </>
        ) : loaded ? (
          <span className="flex h-7 w-full items-center justify-center gap-1.5 rounded-(--r-md) text-[12px] font-semibold text-(--on-soft) shadow-[inset_0_0_0_1px_var(--accent-line)]">
            <Check size={12} strokeWidth={2.5} className="shrink-0" />
            <span className="leading-none">In use</span>
          </span>
        ) : (
          <button
            type="button"
            onClick={installed ? onUse : onDownload}
            disabled={disabled}
            title={download?.error ?? undefined}
            className={`btn btn-sm w-full ${download?.status === 'error' ? 'btn-quiet text-(--danger)' : 'btn-quiet'}`}
          >
            {installed ? (
              'Use this model'
            ) : (
              <>
                <Download size={12} strokeWidth={2} />
                {download?.status === 'error' ? 'Retry download' : 'Download'}
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
      {/* What is running right now. */}
      <div className="panel flex items-center gap-3 px-3.5 py-2.5">
        <span className="grid size-8 shrink-0 place-items-center rounded-(--r-md) bg-(--surface) shadow-[inset_0_0_0_1px_var(--hairline)]">
          {shownVendor
            ? <VendorMark vendor={shownVendor} className="size-4.5" />
            : <Mic size={14} strokeWidth={1.9} className="text-(--muted)" />}
        </span>

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate text-[13px] font-semibold tracking-[-0.015em] text-(--fg)">
              {shownModel ? shownModel.displayName : 'No model loaded'}
            </span>
            {shownModel && isStreaming(shownModel) && (
              <Tag on>
                <Radio size={9} strokeWidth={2.5} />
                streaming
              </Tag>
            )}
          </div>
          {/* Only the empty state gets a second line. A loaded model's blurb
              repeated what the name and the streaming tag already said. */}
          {!shownModel && (
            <p className="m-0 mt-0.5 truncate text-[11.5px] text-(--muted)">
              Pick a model below to start transcribing.
            </p>
          )}
        </div>

        <div className="flex shrink-0 items-center gap-4 text-[11px]">
          <span className="flex items-center gap-1.5 text-(--fg-2)">
            <Cpu size={12} strokeWidth={1.9} className="text-(--on-soft)" />
            {profile
              ? `${profile.executionProvider.toUpperCase()}${profile.vramGb > 0 ? ` · ${profile.vramGb} GB` : ''}`
              : 'Detecting…'}
          </span>
          <span className="rule h-3.5 w-px" />
          <span className="flex items-center gap-1.5 text-(--muted)">
            <HardDrive size={12} strokeWidth={1.9} />
            {onDisk.length} on disk, {(diskBytes / 1e9).toFixed(2)} GB
          </span>
          <button type="button" onClick={() => setManagerOpen(true)} className="btn btn-sm btn-quiet">
            <Database size={11} strokeWidth={1.9} />
            Manage
          </button>
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-2">
        {/* Chips, not a segmented control: these narrow the grid rather than
            switching between views, so each stands on its own. */}
        <div className="flex items-center gap-1.5">
          {FILTERS.map((f) => (
            <button
              key={f.id}
              type="button"
              onClick={() => setFilter(f.id)}
              data-state={filter === f.id ? 'active' : 'inactive'}
              aria-pressed={filter === f.id}
              className="chip"
            >
              {f.label}
              <span className="chip-count">{filterCount(f.id)}</span>
            </button>
          ))}
        </div>

        <div className="relative ml-auto w-48 shrink-0">
          <Search size={12} strokeWidth={2} className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-(--faint)" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Find a model…"
            aria-label="Find a model"
            className="h-7 pl-7 text-[11.5px]"
          />
        </div>
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
        <p className="py-8 text-center text-[12px] text-(--muted)">
          No models match that filter.
        </p>
      )}


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
    </div>
  )
}
