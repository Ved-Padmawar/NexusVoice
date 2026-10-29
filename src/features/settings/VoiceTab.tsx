import { useState, useCallback, useEffect, useMemo, useRef } from 'react'
import {
  ArrowClockwiseIcon, CaretRightIcon, CheckIcon, CpuIcon, DownloadSimpleIcon, HardDrivesIcon,
  HourglassMediumIcon, PlayIcon, WarningCircleIcon, XIcon,
} from '@phosphor-icons/react'
import { commands } from '../../shared/bindings'
import { formatModelSize, isStreaming, modelNameToId, type CatalogModel, type ModelId } from '../models/models'
import { toast } from 'sonner'
import { Button } from '../../shared/ui/Button'
import { Popover } from '../../shared/ui/Popover'
import { Segmented } from '../../shared/ui/Segmented'
import { VendorMark } from '../../shared/ui/VendorMark'
import { ModelManagerModal } from '../models/ModelManagerModal'
import { vendorForFamily } from '../../shared/vendors'
import { cn } from '../../shared/utils'
import type { DownloadedModel, HardwareProfile } from '../../shared/types'
import {
  NO_MODELS, cancelDownload, startDownload, switchModel, useActiveModel, useCatalog, useDownload, usePendingDownloads,
} from '../models/queries'
import { ModelUnloadSection } from './ModelUnloadSection'

const VENDOR_NAME: Record<string, string> = {
  openai: 'OpenAI', nvidia: 'NVIDIA', ibm: 'IBM', qwen: 'Qwen', moonshine: 'Moonshine',
}

/** Quantised — a proportional bar made small models invisible slivers. */
function weightTier(bytes: number): 0 | 1 | 2 | 3 {
  if (bytes >= 1_000_000_000) return 3
  if (bytes >= 600_000_000) return 2
  if (bytes >= 180_000_000) return 1
  return 0
}

const TIER_LABEL = ['Light', 'Medium', 'Heavy', 'Max'] as const
const TIER_HEIGHT = ['h-1', 'h-1.5', 'h-2.25', 'h-3']

function WeightMeter({ bytes }: { bytes: number }) {
  const tier = weightTier(bytes)
  return (
    <span className="inline-flex h-3 items-end gap-0.5" data-tip={`${TIER_LABEL[tier]} download`}>
      {TIER_HEIGHT.map((h, i) => (
        <span key={h} className={cn('w-0.75 rounded-[1px]', h, i <= tier ? 'bg-fg-2' : 'bg-line')} />
      ))}
    </span>
  )
}

const ACTION = 'flex-1 hover:bg-accent-soft hover:text-accent-ink hover:ring-accent-line [&_svg]:text-accent-ink'

/** The card's own description, with the full text in a popover when it is clipped. */
function Description({ model }: { model: CatalogModel }) {
  const ref = useRef<HTMLParagraphElement>(null)
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const [open, setOpen] = useState(false)
  const vendor = vendorForFamily(model.family)

  useEffect(() => () => clearTimeout(timer.current), [])

  const show = () => {
    clearTimeout(timer.current)
    timer.current = setTimeout(() => {
      const el = ref.current
      if (el && el.scrollHeight > el.clientHeight + 1) setOpen(true)
    }, 300)
  }
  const hide = () => {
    clearTimeout(timer.current)
    setOpen(false)
  }

  return (
    <>
      <p
        ref={ref}
        tabIndex={0}
        onPointerEnter={show}
        onPointerLeave={hide}
        onFocus={show}
        onBlur={hide}
        className="line-clamp-2 min-h-[2lh] rounded-xs text-xs text-pretty text-muted"
      >
        {model.detail}
      </p>
      <Popover open={open} onOpenChange={setOpen} anchorRef={ref} className="w-72 p-3">
        <div className="flex items-center gap-2.5">
          <span className="grid size-8 shrink-0 place-items-center rounded-lg bg-field">
            {vendor ? <VendorMark vendor={vendor} className="size-4.5" /> : <CpuIcon size={16} className="text-muted" />}
          </span>
          <div className="min-w-0">
            <p className="truncate text-sm font-semibold text-fg">{model.displayName}</p>
            {vendor && <p className="text-2xs text-muted">{VENDOR_NAME[vendor]}</p>}
          </div>
        </div>
        <p className="mt-2.5 text-xs leading-normal text-fg-2">{model.detail}</p>
      </Popover>
    </>
  )
}

function ModelCard({
  model,
  loaded,
  installed,
  recommended,
  disabled,
  onDownload,
  onUse,
  onCancel,
}: {
  model: CatalogModel
  loaded: boolean
  installed: boolean
  recommended: boolean
  disabled: boolean
  onDownload: () => void
  onUse: () => void
  onCancel: () => void
}) {
  // Per-card subscription: a progress tick re-renders only this card.
  const download = useDownload(model.id).data
  const streaming = isStreaming(model)
  const vendor = vendorForFamily(model.family)
  const failed = download?.status === 'error'

  return (
    <article
      className={cn(
        'flex min-w-0 flex-col gap-3 rounded-2xl bg-panel p-4 ring-inset transition-shadow duration-200',
        loaded ? 'ring-2 ring-accent' : 'ring-1 ring-line-soft',
      )}
    >
      <header className="flex items-start gap-3">
        <span className="grid size-10 shrink-0 place-items-center rounded-xl bg-field ring-1 ring-line-soft ring-inset">
          {vendor ? <VendorMark vendor={vendor} className="size-5" /> : <CpuIcon size={18} className="text-muted" />}
        </span>
        <div className="min-w-0 flex-1">
          <p className="flex items-center gap-1.5 text-2xs text-muted">
            {vendor && VENDOR_NAME[vendor]}
            {streaming && <span className="rounded-md bg-accent-2-soft px-1.5 font-semibold text-accent-2-ink">Streaming</span>}
            {recommended && <span className="rounded-md bg-accent-soft px-1.5 font-semibold text-accent-ink">Recommended</span>}
          </p>
          <h3 className="mt-0.5 truncate text-sm font-semibold text-fg" title={model.displayName}>{model.displayName}</h3>
        </div>
      </header>

      <Description model={model} />

      <dl className="grid grid-cols-2 gap-2 rounded-xl bg-field px-3 py-2">
        <div>
          <dt className="text-2xs text-muted">Size</dt>
          <dd className="mt-0.5 flex items-baseline gap-1.5 text-xs font-semibold text-fg tabular-nums">
            {formatModelSize(model.sizeBytes)}
            <WeightMeter bytes={model.sizeBytes} />
          </dd>
        </div>
        <div>
          <dt className="text-2xs text-muted">Languages</dt>
          <dd className="mt-0.5 text-xs font-semibold text-fg">{model.multilingual ? 'Multilingual' : 'English'}</dd>
        </div>
      </dl>

      {failed && download.error && (
        <p className="-mt-1 truncate text-2xs text-danger" title={download.error}>
          <WarningCircleIcon size={11} weight="fill" className="inline align-[-1px]" /> {download.error}
        </p>
      )}

      <footer className="mt-auto flex h-8 items-center gap-2">
        {installed ? (
          loaded ? (
            <span className="inline-flex h-8 flex-1 items-center justify-center gap-1.5 rounded-lg bg-accent text-xs font-semibold text-accent-fg">
              <CheckIcon size={12} weight="bold" />In use
            </span>
          ) : (
            <Button variant="chip" onClick={onUse} disabled={disabled} className={ACTION}>
              <PlayIcon size={12} weight="bold" />Use
            </Button>
          )
        ) : download && !failed ? (
          <div className="flex h-8 flex-1 items-center gap-2.5">
            {download.status === 'queued' ? (
              <span className="flex flex-1 items-center gap-1.5 text-xs font-semibold text-fg-2">
                <HourglassMediumIcon size={14} className="queue-glass text-accent-ink" />Up next
              </span>
            ) : (
              <>
                <DownloadSimpleIcon size={14} weight="bold" className="shrink-0 text-accent-ink" />
                <span className="h-1.5 flex-1 overflow-hidden rounded-full bg-fg/10">
                  <span className="block h-full rounded-full bg-accent transition-[width] duration-300" style={{ width: `${download.progress}%` }} />
                </span>
                <span className="w-8 text-right text-xs font-semibold text-fg-2 tabular-nums">{download.progress}%</span>
              </>
            )}
            <button
              type="button"
              aria-label="Cancel download"
              onClick={onCancel}
              className="grid size-6 shrink-0 cursor-pointer place-items-center rounded-md text-muted hover:bg-danger-soft hover:text-danger"
            >
              <XIcon size={12} weight="bold" />
            </button>
          </div>
        ) : (
          <Button
            variant="chip"
            onClick={onDownload}
            disabled={disabled}
            className={cn(ACTION, failed && 'text-danger ring-danger/40 hover:bg-danger-soft hover:text-danger hover:ring-danger/60 [&_svg]:text-danger')}
          >
            {failed ? <ArrowClockwiseIcon size={12} weight="bold" /> : <DownloadSimpleIcon size={12} weight="bold" />}
            {failed ? 'Retry' : 'Download'}
            <span className="font-normal tabular-nums opacity-70">{formatModelSize(model.sizeBytes)}</span>
          </Button>
        )}
      </footer>
    </article>
  )
}

type Filter = 'all' | 'installed' | 'streaming' | 'multilingual'

const FILTER_LABEL: Record<Filter, string> = {
  all: 'All', installed: 'On this PC', streaming: 'Streaming', multilingual: 'Multilingual',
}

/** The model catalog, filtered to what the chosen tab matches. */
export function VoiceTab() {
  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [modelSaving, setModelSaving] = useState(false)
  const [onDisk, setOnDisk] = useState<DownloadedModel[]>([])
  const [managerOpen, setManagerOpen] = useState(false)
  const [filter, setFilter] = useState<Filter>('all')

  const catalog = useCatalog().data ?? NO_MODELS
  const pendingCount = usePendingDownloads().data
  const selected = useActiveModel().data?.id ?? null

  const refreshOnDisk = useCallback(() => {
    commands.getDownloadedModels().then(setOnDisk).catch(() => setOnDisk([]))
  }, [])

  useEffect(() => {
    commands.getHardwareProfile().then(setProfile).catch(() => {})
  }, [])

  // Reads the disk on mount, and again when a finished download adds a file.
  useEffect(() => { refreshOnDisk() }, [pendingCount, refreshOnDisk])

  const onDiskVariants = useMemo(() => new Set(onDisk.map(m => m.variant)), [onDisk])
  const recommendedVariant = profile ? modelNameToId(profile.recommendedModel, catalog) : null
  // `selected` is set even with nothing downloaded, so require the file too.
  const loadedVariant = selected && onDiskVariants.has(selected) ? selected : null

  // In use first, then what is on disk, then the rest; smallest first within each.
  const ordered = useMemo(() => {
    const rank = (m: CatalogModel) => (m.id === loadedVariant ? 0 : onDiskVariants.has(m.id) ? 1 : 2)
    return [...catalog].sort((a, b) => rank(a) - rank(b) || a.sizeBytes - b.sizeBytes)
  }, [catalog, loadedVariant, onDiskVariants])

  const diskBytes = onDisk.reduce((a, m) => a + m.sizeBytes, 0)

  const matches = useCallback((m: CatalogModel, f: Filter) =>
    f === 'all' || (f === 'installed' ? onDiskVariants.has(m.id) : f === 'streaming' ? isStreaming(m) : m.multilingual),
  [onDiskVariants])

  const shown = ordered.filter(m => matches(m, filter))
  const filterOptions = (Object.keys(FILTER_LABEL) as Filter[]).map(f => ({
    value: f,
    label: <>{FILTER_LABEL[f]} <span className="tabular-nums opacity-70">{ordered.filter(m => matches(m, f)).length}</span></>,
  }))

  // Only offered for a model on disk, so the override can't point at nothing.
  const handleUseModel = async (v: ModelId) => {
    setModelSaving(true)
    try {
      await switchModel(v)
      toast.success('Model updated')
    } catch { /* ignore */ }
    finally { setModelSaving(false) }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-3">
        <Segmented value={filter} options={filterOptions} onChange={setFilter} aria-label="Filter models" />
        <Button variant="chip" onClick={() => setManagerOpen(true)} className="group ml-auto">
          <HardDrivesIcon size={14} weight="bold" className="text-accent-ink" />
          Manage downloads
          <span className="font-normal text-muted tabular-nums">{formatModelSize(diskBytes)}</span>
          <CaretRightIcon size={10} weight="bold" className="text-muted transition-transform group-hover:translate-x-0.5" />
        </Button>
        <ModelUnloadSection />
      </div>

      {/* Room for the in-use card's outline and the last row; the scrollbar sits
          in the page's right gutter instead of over the cards. */}
      <div className="voice-scroll mt-4 -mr-3.5 min-h-0 flex-1 overflow-y-auto overscroll-none pt-0.5 pr-1 pb-8 [scrollbar-gutter:stable]">
        {shown.length === 0 && (
          <p className="py-16 text-center text-sm text-muted">No models here yet.</p>
        )}
        <div className="grid grid-cols-3 gap-3">
          {shown.map((model) => (
            <ModelCard
              key={model.id}
              model={model}
              loaded={loadedVariant === model.id}
              installed={onDiskVariants.has(model.id)}
              recommended={recommendedVariant === model.id}
              disabled={modelSaving}
              onDownload={() => void startDownload(model.id)}
              onUse={() => void handleUseModel(model.id)}
              onCancel={() => void cancelDownload(model.id)}
            />
          ))}
        </div>
      </div>

      <ModelManagerModal
        open={managerOpen}
        onClose={() => {
          setManagerOpen(false)
          refreshOnDisk()
        }}
      />
    </div>
  )
}
