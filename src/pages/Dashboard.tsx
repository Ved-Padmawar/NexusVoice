import { useState, useRef, useEffect, useCallback, useMemo } from 'react'
import { Popover } from 'radix-ui'
import {
  Copy, Check, Trash2, Search, Download, SlidersHorizontal, Mic,
  Hash, Timer, Gauge, Cpu, PackageOpen, X, FileText, FileJson,
  Keyboard, Sparkles, Settings2,
} from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from '../lib/commands'
import { toast } from 'sonner'
import { extractErrorMessage } from '../lib/errors'
import { useAppStore } from '../store/useAppStore'
import {
  useTranscripts,
  useTranscriptSearch,
  useStats,
  useDeleteTranscript,
  NO_FILTERS,
  type TranscriptFilters,
} from '../lib/queries'
import { VendorMark } from '../components/ui/VendorMark'
import { RangeCalendar } from '../components/RangeCalendar'
import { isoDay, daysAgo, dayLabel } from '../lib/dates'
import { vendorForFamily } from '../lib/vendors'
import { fmtTime, fmtDate, downloadBlob } from '../lib/utils'
import { Input } from '@/components/ui/input'
import { PageBar } from '../components/PageBar'
import { SectionState } from '../components/SectionState'
import { Spinner } from '../components/Spinner'
import type { Transcript } from '../types'

/** Popover chrome, shared by the two menus in the page bar. */
const POPOVER =
  'pop z-50 origin-top-right overflow-hidden rounded-(--r-md) ' +
  'data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 ' +
  'data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95'

function ReadoutSkeleton() {
  return (
    <div className="flex items-end gap-x-12">
      {[0, 1, 2, 3].map(i => (
        <div key={i} className="flex flex-col gap-2.5">
          <div className="h-7 w-20 animate-pulse rounded bg-(--surface)" />
          <div className="h-2.5 w-16 animate-pulse rounded bg-(--surface)" />
        </div>
      ))}
    </div>
  )
}

function FeedSkeleton() {
  return (
    <div className="flex flex-col">
      {[0, 1, 2, 3, 4].map(i => (
        <div key={i} className="flex flex-col gap-2 border-b border-(--hairline) py-3.5 last:border-0">
          <div className="h-3 w-full animate-pulse rounded bg-(--surface)" />
          <div className="h-2.5 w-24 animate-pulse rounded bg-(--surface)" />
        </div>
      ))}
    </div>
  )
}

function ExportMenu() {
  const [open, setOpen] = useState(false)
  const [exporting, setExporting] = useState(false)

  const doExport = async (format: 'txt' | 'json') => {
    setOpen(false)
    setExporting(true)
    try {
      const items = await invoke<Transcript[]>(COMMANDS.EXPORT_TRANSCRIPTS)
      const date = new Date().toISOString().slice(0, 10)
      if (format === 'txt') {
        const content = items.map(t => `[${fmtDate(t.createdAt)}]\n${t.content}`).join('\n\n---\n\n')
        downloadBlob(content, `nexusvoice-transcripts-${date}.txt`, 'text/plain')
        toast.success(`Exported ${items.length} transcript${items.length !== 1 ? 's' : ''} as TXT`)
      } else {
        const content = JSON.stringify(items.map(t => ({ id: t.id, content: t.content, createdAt: t.createdAt, wordCount: t.wordCount, durationSeconds: t.durationSeconds })), null, 2)
        downloadBlob(content, `nexusvoice-transcripts-${date}.json`, 'application/json')
        toast.success(`Exported ${items.length} transcript${items.length !== 1 ? 's' : ''} as JSON`)
      }
    } catch (e) {
      toast.error(extractErrorMessage(e, 'Export failed'))
    } finally {
      setExporting(false)
    }
  }

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button type="button" disabled={exporting} title="Export transcripts" className="btn btn-sm btn-quiet">
          <Download size={11} strokeWidth={2} />
          Export
        </button>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content align="end" sideOffset={6} className={`${POPOVER} w-44`}>
          {([
            ['txt', 'Plain text', '.txt', FileText],
            ['json', 'JSON', '.json', FileJson],
          ] as const).map(([fmt, label, ext, Icon]) => (
            <button
              key={fmt}
              type="button"
              onClick={() => doExport(fmt)}
              className="flex h-8 w-full cursor-pointer items-center gap-2.5 px-3 text-left text-[12px] text-(--fg-2) transition-colors duration-(--t-fast) hover:bg-(--surface-hover) hover:text-(--fg)"
            >
              <Icon size={13} strokeWidth={1.9} className="shrink-0 text-(--muted)" />
              {label}
              <span className="ml-auto text-[10.5px] tabular-nums text-(--faint)">{ext}</span>
            </button>
          ))}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  )
}

const STATS = [
  { key: 'totalWords',          label: 'Words',            Icon: Hash,  fmt: (v: number) => v.toLocaleString() },
  { key: 'speakingTimeSeconds', label: 'Speaking time',    Icon: Timer, fmt: (v: number) => fmtTime(v) },
  { key: 'totalSessions',       label: 'Sessions',         Icon: Mic,   fmt: (v: number) => v.toLocaleString() },
  { key: 'avgPaceWpm',          label: 'Words per minute', Icon: Gauge, fmt: (v: number) => `${v}` },
]

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => () => { if (timerRef.current) clearTimeout(timerRef.current) }, [])

  const handleCopy = () => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      if (timerRef.current) clearTimeout(timerRef.current)
      timerRef.current = setTimeout(() => setCopied(false), 2000)
    }).catch(() => toast.error('Could not copy to the clipboard'))
  }
  return (
    <button
      type="button"
      onClick={handleCopy}
      title={copied ? 'Copied' : 'Copy transcript'}
      aria-label="Copy transcript"
      className={`iconbtn ${copied ? 'text-(--success)' : 'iconbtn-accent'}`}
    >
      {copied ? <Check size={13} strokeWidth={2.5} /> : <Copy size={13} strokeWidth={1.9} />}
    </button>
  )
}

/**
 * Almost every filter is a recent window, so those are one click across the
 * top. The calendar underneath handles everything else, including spans months
 * apart, which a pair of text fields served worst.
 */
const PRESETS = [
  { id: 'today', label: 'Today',   resolve: () => ({ from: isoDay(new Date()), to: isoDay(new Date()) }) },
  { id: '7d',    label: '7 days',  resolve: () => ({ from: daysAgo(6),  to: isoDay(new Date()) }) },
  { id: '30d',   label: '30 days', resolve: () => ({ from: daysAgo(29), to: isoDay(new Date()) }) },
] as const

function FilterMenu({ filters, onChange }: {
  filters: TranscriptFilters
  onChange: (filters: TranscriptFilters) => void
}) {
  const [open, setOpen] = useState(false)
  const [from, setFrom] = useState(filters.from)
  const [to, setTo] = useState(filters.to)
  const [sortAsc, setSortAsc] = useState(filters.sortAsc)

  const activeCount = (filters.from || filters.to ? 1 : 0) + (filters.sortAsc ? 1 : 0)

  const handleOpenChange = (next: boolean) => {
    if (next) {
      setFrom(filters.from); setTo(filters.to); setSortAsc(filters.sortAsc)
    }
    setOpen(next)
  }

  // Every control commits on the spot. The panel stays open so a date range and
  // an order can be set in one visit, and the list updates behind it.
  const commit = (next: Partial<TranscriptFilters>) => {
    const merged = { from, to, sortAsc, ...next }
    setFrom(merged.from); setTo(merged.to); setSortAsc(merged.sortAsc)
    onChange(merged)
  }

  const presetActive = (id: typeof PRESETS[number]['id']) => {
    const p = PRESETS.find(x => x.id === id)
    return !!p && from === p.resolve().from && to === p.resolve().to
  }

  const clear = () => {
    setFrom(null); setTo(null); setSortAsc(false)
    onChange(NO_FILTERS)
  }

  return (
    <Popover.Root open={open} onOpenChange={handleOpenChange}>
      <Popover.Trigger asChild>
        <button type="button" className="btn btn-sm btn-quiet">
          <SlidersHorizontal size={11} strokeWidth={2} />
          Filter
          {activeCount > 0 && (
            <span className="grid size-4 place-items-center rounded-full bg-(--accent) text-[9.5px] font-bold text-(--accent-fg)">
              {activeCount}
            </span>
          )}
        </button>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content align="end" sideOffset={6} className={`${POPOVER} w-72`}>
          <div className="flex items-center gap-1.5 p-3">
            {PRESETS.map(p => {
              const on = presetActive(p.id)
              return (
                <button
                  key={p.id}
                  type="button"
                  onClick={() => commit(p.resolve())}
                  aria-pressed={on}
                  className={`h-7 flex-1 rounded-(--r-sm) text-[11.5px] font-medium transition-colors duration-(--t-fast) ${
                    on
                      ? 'bg-(--accent-soft) text-(--on-soft) shadow-[inset_0_0_0_1px_var(--accent-line)]'
                      : 'bg-(--surface) text-(--fg-2) hover:bg-(--surface-hover) hover:text-(--fg)'
                  }`}
                >
                  {p.label}
                </button>
              )
            })}
          </div>

          <div className="rule h-px" />

          <div className="p-3">
            <RangeCalendar from={from} to={to} onChange={(f, t) => commit({ from: f, to: t })} />
          </div>

          {/* Reads back the committed span, so the grid is not the only record. */}
          <div className="rule h-px" />
          <div className="flex h-9 items-center gap-2 px-3">
            <span className="min-w-0 flex-1 truncate text-[11.5px] text-(--fg-2)">
              {from && to
                ? (from === to ? dayLabel(from) : `${dayLabel(from)} – ${dayLabel(to)}`)
                : <span className="text-(--muted)">Any time</span>}
            </span>
            {activeCount > 0 && (
              <button
                type="button" onClick={clear}
                className="flex items-center gap-1 text-[11px] text-(--muted) transition-colors duration-(--t-fast) hover:text-(--danger)"
              >
                <X size={11} strokeWidth={2.25} />
                Clear
              </button>
            )}
          </div>

          <div className="rule h-px" />
          <div className="flex items-center gap-2 p-3">
            <span className="text-[11px] text-(--muted)">Order</span>
            <div className="seg ml-auto">
              {([false, true] as const).map(asc => (
                <button
                  key={String(asc)}
                  type="button"
                  onClick={() => commit({ sortAsc: asc })}
                  data-state={sortAsc === asc ? 'active' : 'inactive'}
                  className="seg-item"
                >
                  {asc ? 'Oldest first' : 'Newest first'}
                </button>
              ))}
            </div>
          </div>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  )
}

/**
 * Which model is loaded and what to do next.
 *
 * When nothing is downloaded the catalogue still hands back a name — the
 * recommended model, not one in hand — so leading with it read as "you have
 * Whisper Large v3 Turbo" while the line beside it told you to go get a model.
 * The name leads only when it is genuinely loaded; otherwise it is demoted to
 * the recommendation it actually is.
 */
function ModelLine() {
  const hasHotkey = useAppStore(s => s.hasHotkey)
  const modelReady = useAppStore(s => s.modelReady)
  const modelName = useAppStore(s => s.activeModelName)
  const catalog = useAppStore(s => s.catalog)
  const selected = useAppStore(s => s.selectedModel)
  const downloads = useAppStore(s => s.downloads)

  // The mark of whoever trained the loaded model, matching the Voice tab.
  const model = catalog.find(m => m.id === selected)
  const vendor = model ? vendorForFamily(model.family) : null

  const download = selected ? downloads[selected] : undefined
  const fetching = download?.status === 'running' || download?.status === 'queued'

  // The mark says what is loaded; the detail icon says what to do about it, so
  // the two never carry the same message. A model that is only recommended
  // keeps the neutral mark — showing its vendor logo is what made the line read
  // as "you have this one".
  const { mark, DetailIcon, title, detail } = fetching
    ? {
        mark: vendor
          ? <VendorMark vendor={vendor} className="size-5.5" />
          : <Download size={17} strokeWidth={1.9} className="text-(--accent)" />,
        DetailIcon: Download,
        title: modelName ?? 'Model',
        detail: `Downloading · ${download?.progress ?? 0}%`,
      }
    : !modelReady
      ? {
          mark: <PackageOpen size={17} strokeWidth={1.9} className="text-(--muted)" />,
          DetailIcon: modelName ? Sparkles : Settings2,
          title: 'No model',
          detail: modelName ? `${modelName} recommended` : 'Choose one in Settings',
        }
      : {
          mark: vendor
            ? <VendorMark vendor={vendor} className="size-5.5" />
            : <Cpu size={17} strokeWidth={1.9} className="text-(--muted)" />,
          DetailIcon: hasHotkey ? Mic : Keyboard,
          title: modelName ?? 'Model loaded',
          detail: hasHotkey ? 'Hold your hotkey to record' : 'Set a hotkey in Settings',
        }

  return (
    <div className="flex min-w-0 items-center gap-2.5">
      <span className="grid size-6 shrink-0 place-items-center">{mark}</span>
      <span className="truncate text-[12px] font-medium text-(--fg-2)">{title}</span>
      <span className="rule h-3 w-px shrink-0" />
      <span className="flex min-w-0 items-center gap-1.5 text-[11.5px] tabular-nums text-(--muted)">
        <DetailIcon size={12} strokeWidth={1.9} className="shrink-0" />
        <span className="truncate">{detail}</span>
      </span>
    </div>
  )
}

export function Dashboard() {
  const [query, setQuery] = useState('')
  const [searchTerm, setSearchTerm] = useState('')
  const [filters, setFilters] = useState<TranscriptFilters>(NO_FILTERS)
  const observerRef = useRef<IntersectionObserver | null>(null)

  const isSearchMode = searchTerm.length > 0
  const feed = useTranscripts(filters, !isSearchMode)
  const search = useTranscriptSearch(searchTerm, filters)
  const active = isSearchMode ? search : feed
  const stats = useStats()
  const deleteTranscript = useDeleteTranscript()

  const displayItems = useMemo(() => {
    const rows = active.data?.pages.flat() ?? []
    const seen = new Set<number>()
    return rows.filter(row => !seen.has(row.id) && seen.add(row.id))
  }, [active.data])
  const feedCount = useMemo(
    () => new Set(feed.data?.pages.flat().map(row => row.id) ?? []).size,
    [feed.data],
  )

  // A ref callback, not an effect: the sentinel mounts only after the feed's
  // skeleton is replaced, which changes no effect dependency — an effect would
  // read a null ref and never re-run.
  const { fetchNextPage, hasNextPage, isFetching } = active
  const canFetchNext = hasNextPage && !isFetching
  const sentinelRef = useCallback((node: HTMLDivElement | null) => {
    observerRef.current?.disconnect()
    if (!node || !canFetchNext) return
    observerRef.current = new IntersectionObserver(
      (entries) => { if (entries[0].isIntersecting) void fetchNextPage() },
      { threshold: 0.1 }
    )
    observerRef.current.observe(node)
  }, [canFetchNext, fetchNextPage])

  useEffect(() => () => observerRef.current?.disconnect(), [])

  useEffect(() => {
    const id = setTimeout(() => setSearchTerm(query.trim()), 300)
    return () => clearTimeout(id)
  }, [query])

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <PageBar
        title="Dashboard"
        description="Your recent dictation, at a glance"
      />

      <div className="mx-auto mb-(--dock-clear) flex min-h-0 w-full max-w-(--measure) flex-1 flex-col overflow-hidden px-(--gutter)">

        {/* Four numbers on the bare canvas, set large. Boxing figures this size
            adds a container the eye parses before reaching the value; the rule
            beneath is all the separation the feed needs. */}
        <div className="flex shrink-0 flex-wrap items-end gap-x-8 gap-y-4 pb-5">
          <SectionState
            status={stats.status}
            error={stats.error?.message}
            onRetry={stats.refetch}
            skeleton={<ReadoutSkeleton />}
            hasData={stats.data != null}
          >
            <div className="flex flex-wrap items-end gap-x-12 gap-y-4">
              {STATS.map(({ key, label, Icon, fmt }) => {
                const raw = stats.data?.[key as keyof typeof stats.data] as number | undefined
                return (
                  <div key={key} className="flex flex-col gap-2">
                    <span className="text-[30px] font-semibold leading-none tracking-[-0.045em] tabular-nums text-(--fg)">
                      {raw != null ? fmt(raw) : '—'}
                    </span>
                    <span className="flex items-center gap-1.5 text-[11px] text-(--muted)">
                      <Icon size={12} strokeWidth={1.9} className="shrink-0" />
                      {label}
                    </span>
                  </div>
                )
              })}
            </div>
          </SectionState>

          {/* Outside the readouts' loading state: neither the model nor the next
              action waits on the stats query. */}
          <div className="ml-auto min-w-0 pb-0.5">
            <ModelLine />
          </div>
        </div>

        <div className="rule h-px shrink-0" />

        <div className="flex min-h-0 flex-1 flex-col overflow-hidden pt-4">
          <div className="flex shrink-0 items-center gap-2.5 pb-3 pr-3.5">
            <h2 className="m-0 text-[12px] font-semibold tracking-[-0.005em] text-(--fg-2)">
              {isSearchMode ? 'Search results' : 'Recent activity'}
            </h2>
            {!isSearchMode && feedCount > 0 && (
              <span className="count">{feedCount}</span>
            )}

            <div className="ml-auto flex items-center gap-2">
              <div className="relative flex items-center">
                <Search size={12} strokeWidth={2} className="pointer-events-none absolute left-2.5 text-(--faint)" />
                <Input
                  value={query}
                  onChange={e => setQuery(e.target.value)}
                  placeholder="Search transcripts…"
                  aria-label="Search transcripts"
                  className="h-7 w-52 pl-7 text-[12px]"
                />
              </div>
              <FilterMenu filters={filters} onChange={setFilters} />
              <ExportMenu />
            </div>
          </div>

          <SectionState
            status={active.status}
            error={active.error?.message}
            onRetry={active.refetch}
            skeleton={<FeedSkeleton />}
          >
            {displayItems.length === 0 ? (
              <div className="flex flex-col items-center gap-2.5 px-6 py-16 text-center">
                <span className="grid size-10 place-items-center rounded-full bg-(--surface) text-(--faint)">
                  {isSearchMode ? <Search size={17} strokeWidth={1.6} /> : <Mic size={17} strokeWidth={1.6} />}
                </span>
                <p className="m-0 text-[13px] font-semibold text-(--fg-2)">
                  {isSearchMode ? 'No results found' : 'Nothing here yet'}
                </p>
                <p className="m-0 max-w-64 text-[12px] leading-[1.6] text-(--muted)">
                  {isSearchMode
                    ? 'Try a different word, or check the spelling.'
                    : 'Hold your hotkey and speak. Transcripts land here as you go.'}
                </p>
              </div>
            ) : (
              <div className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden overscroll-none pr-1.5">
                {displayItems.map((item) => (
                  <article
                    key={item.id}
                    className="group flex gap-4 border-b border-(--hairline) py-3.5 last:border-0"
                  >
                    <div className="min-w-0 flex-1">
                      <p data-selectable className="m-0 text-[13px] leading-[1.55] text-(--fg)">{item.content}</p>
                      <div className="mt-1.5 flex items-center gap-2.5 text-[10.5px] text-(--muted)">
                        <span className="tabular-nums">{fmtDate(item.createdAt)}</span>
                        {item.targetApp && (
                          <>
                            <span className="rule h-2.5 w-px" />
                            <span className="truncate">Pasted in {item.targetApp}</span>
                          </>
                        )}
                      </div>
                    </div>
                    <div className="row-actions flex shrink-0 items-start gap-0.5">
                      <CopyButton text={item.content} />
                      <button
                        type="button"
                        onClick={() => deleteTranscript.mutate(item.id)}
                        title="Delete transcript"
                        aria-label="Delete transcript"
                        className="iconbtn iconbtn-danger"
                      >
                        <Trash2 size={13} strokeWidth={1.9} />
                      </button>
                    </div>
                  </article>
                ))}

                {/* Infinite scroll sentinel — pages the feed and search alike. */}
                {hasNextPage && (
                  <div ref={sentinelRef} className="flex items-center justify-center py-5">
                    <Spinner size={16} />
                  </div>
                )}
              </div>
            )}
          </SectionState>
        </div>
      </div>
    </div>
  )
}
