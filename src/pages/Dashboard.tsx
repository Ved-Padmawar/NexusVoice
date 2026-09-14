import { useState, useRef, useEffect, useCallback, useMemo } from 'react'
import { useNavigate } from 'react-router'
import { AnimatePresence, motion } from 'framer-motion'
import { Popover } from 'radix-ui'
import {
  AppWindow, Check, Copy, Download, FileJson, FileText, Gauge, Hash, Keyboard,
  Mic, Search, SlidersHorizontal, Timer, Trash2, Type,
} from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { COMMANDS } from '../lib/commands'
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
import { ROUTES, SETTINGS_TABS, type SettingsTab } from '../lib/routes'
import { useRegisteredHotkeys } from '../lib/hotkeys'
import { isStreaming } from '../lib/models'
import { vendorForFamily } from '../lib/vendors'
import { fmtTime, fmtDate, fmtClock, dayOf, downloadBlob, type DayGroup } from '../lib/utils'
import { Button, IconButton } from '@/components/ui/button'
import { SearchInput } from '@/components/ui/input'
import { KeyCombo } from '@/components/ui/keycap'
import { PopoverPanel } from '@/components/ui/popover'
import { Segmented } from '@/components/ui/segmented'
import { VendorMark } from '@/components/ui/VendorMark'
import { StickyBar } from '../components/page'
import { SectionState } from '../components/SectionState'
import type { Transcript } from '../types'

/** Speech-shaped bars behind the hero. */
const WAVE = Array.from({ length: 52 }, (_, i) => {
  const envelope = Math.sin((Math.PI * i) / 51) ** 0.7
  const syllable = 0.28 + 0.72 * Math.abs(Math.sin(i * 1.71) * Math.cos(i * 0.47))
  return Math.max(0.05, envelope * syllable)
})

function HeroWave() {
  return (
    <svg className="nv-hero__wave" viewBox="0 0 520 200" preserveAspectRatio="none" aria-hidden>
      {WAVE.map((v, i) => {
        const h = v * 180
        return <rect key={i} x={i * 10} y={100 - h / 2} width={5} height={h} rx={2.5} fill="currentColor" />
      })}
    </svg>
  )
}

function useOpenSettings() {
  const navigate = useNavigate()
  return (tab: SettingsTab) => navigate(ROUTES.SETTINGS, { state: { tab } })
}

function ModelStatus() {
  const name = useAppStore(s => s.activeModelName)
  const downloaded = useAppStore(s => s.activeModelDownloaded)
  const selected = useAppStore(s => s.selectedModel)
  const catalog = useAppStore(s => s.catalog)
  const openSettings = useOpenSettings()

  const model = catalog.find(m => m.id === selected)
  const vendor = model ? vendorForFamily(model.family) : null
  const ready = name !== null && downloaded

  return (
    <aside className="nv-hero__model">
      <div className="flex min-w-0 flex-col gap-2.5">
        <span className="nv-hero__model-label">
          {name === null ? 'Checking the model…' : ready ? 'Transcribing with' : 'No model on this computer'}
        </span>
        {ready && (
          <span className="nv-hero__model-name">
            <span className="nv-mark nv-mark--sm">
              {vendor ? <VendorMark vendor={vendor} className="size-4" /> : <Mic size={15} />}
            </span>
            <span className="min-w-0">
              {model?.displayName ?? name}
              <span className="block text-[12px] font-normal tracking-normal text-muted">
                {model && isStreaming(model) ? 'Streams text as you speak' : 'Runs on this computer'}
              </span>
            </span>
          </span>
        )}
      </div>
      <Button
        size="sm"
        variant={ready ? 'secondary' : 'primary'}
        className="self-start"
        onClick={() => openSettings(SETTINGS_TABS.VOICE)}
      >
        {ready ? 'Change model' : 'Choose a model'}
      </Button>
    </aside>
  )
}

const STATS = [
  { key: 'totalWords',          label: 'Words dictated', fmt: (v: number) => v.toLocaleString(), unit: null,  Icon: Hash },
  { key: 'speakingTimeSeconds', label: 'Time speaking',  fmt: (v: number) => fmtTime(v),         unit: null,  Icon: Timer },
  { key: 'totalSessions',       label: 'Sessions',       fmt: (v: number) => v.toLocaleString(), unit: null,  Icon: Mic },
  { key: 'avgPaceWpm',          label: 'Average pace',   fmt: (v: number) => `${v}`,             unit: 'wpm', Icon: Gauge },
] as const

function Readout() {
  const stats = useStats()
  const skeleton = STATS.map(({ key }) => (
    <div key={key} className="nv-readout__cell">
      <div className="nv-skel h-6 w-16" />
      <div className="nv-skel mt-2 h-3 w-20" />
    </div>
  ))

  return (
    <div className="nv-readout">
      <SectionState
        status={stats.status}
        error={stats.error?.message}
        onRetry={stats.refetch}
        skeleton={skeleton}
        hasData={stats.data != null}
      >
        {STATS.map(({ key, label, fmt, unit, Icon }) => {
          const raw = stats.data?.[key]
          return (
            <div key={key} className="nv-readout__cell">
              <div className="nv-readout__value">
                <span>{raw != null ? fmt(raw) : '—'}</span>
                {unit && raw != null && <span className="nv-readout__unit">{unit}</span>}
              </div>
              <div className="nv-readout__label"><Icon strokeWidth={2} aria-hidden />{label}</div>
            </div>
          )
        })}
      </SectionState>
    </div>
  )
}

function Hero() {
  const hasHotkey = useAppStore(s => s.hasHotkey)
  const [hotkeys] = useRegisteredHotkeys()
  const openSettings = useOpenSettings()
  const ptt = hotkeys.ptt?.split('+')

  return (
    <section className="nv-card nv-hero">
      <div className="nv-hero__main">
        <HeroWave />
        {!hasHotkey ? (
          <>
            <h1 className="nv-hero__title">Choose a key to hold while you speak</h1>
            <p className="nv-hero__lede">No hotkey set. NexusVoice can't record until you pick one.</p>
            <Button className="nv-hero__cta" onClick={() => openSettings(SETTINGS_TABS.GENERAL)}>
              <Keyboard />
              Set hotkey
            </Button>
          </>
        ) : (
          <>
            <h1 className="nv-hero__title">
              {ptt ? <>Hold <KeyCombo keys={ptt} pressable /> and speak</> : 'Hold your hotkey and speak'}
            </h1>
            <p className="nv-hero__lede">Let go, and the transcript is pasted wherever your cursor is.</p>
            {(hotkeys.dictation || hotkeys.dictationCommit) && (
              <div className="nv-hero__alts">
                {hotkeys.dictation && (
                  <span className="nv-hero__alt">Hands-free dictation <KeyCombo keys={hotkeys.dictation.split('+')} /></span>
                )}
                {hotkeys.dictationCommit && (
                  <span className="nv-hero__alt">Finish dictation <KeyCombo keys={hotkeys.dictationCommit.split('+')} /></span>
                )}
              </div>
            )}
          </>
        )}
      </div>
      <ModelStatus />
      <Readout />
    </section>
  )
}

function ExportButton() {
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
        <Button variant="secondary" disabled={exporting} title="Export transcripts">
          <Download />
          Export
        </Button>
      </Popover.Trigger>
      <PopoverPanel open={open} className="nv-menu min-w-44">
        <button type="button" onClick={() => doExport('txt')} className="nv-menu-item">
          <FileText aria-hidden />Plain text (.txt)
        </button>
        <button type="button" onClick={() => doExport('json')} className="nv-menu-item">
          <FileJson aria-hidden />JSON (.json)
        </button>
      </PopoverPanel>
    </Popover.Root>
  )
}

type DateMode = 'range' | 'on'

function FilterButton({ filters, onChange }: { filters: TranscriptFilters; onChange: (filters: TranscriptFilters) => void }) {
  const { from: filterFrom, to: filterTo, sortAsc: filterSortAsc } = filters
  const [open, setOpen] = useState(false)
  const [dateMode, setDateMode] = useState<DateMode>('range')
  const [from, setFrom] = useState(filterFrom ?? '')
  const [to, setTo] = useState(filterTo ?? '')
  const [on, setOn] = useState('')
  const [sortAsc, setSortAsc] = useState(filterSortAsc)
  const hasActive = !!filterFrom || !!filterTo || filterSortAsc

  const handleOpenChange = (next: boolean) => {
    if (next) {
      setFrom(filterFrom ?? '')
      setTo(filterTo ?? '')
      setSortAsc(filterSortAsc)
      setOn('')
    }
    setOpen(next)
  }

  const apply = () => {
    if (dateMode === 'on' && on) {
      onChange({ from: on, to: on, sortAsc })
    } else {
      onChange({ from: from || null, to: to || null, sortAsc })
    }
    setOpen(false)
  }
  const reset = () => {
    setFrom(''); setTo(''); setOn(''); setSortAsc(false)
    onChange(NO_FILTERS)
    setOpen(false)
  }

  return (
    <Popover.Root open={open} onOpenChange={handleOpenChange}>
      <Popover.Trigger asChild>
        <Button variant="secondary" className={hasActive ? 'text-accent-text!' : undefined}>
          <SlidersHorizontal />
          Filter
          {hasActive && <><span className="nv-dot" aria-hidden /><span className="sr-only">, filters applied</span></>}
        </Button>
      </Popover.Trigger>
      <PopoverPanel open={open} className="w-74">
        <div className="nv-pop-body">
          <div>
            <span className="nv-field-label">Date</span>
            <Segmented
              block
              label="Date filter"
              value={dateMode}
              onChange={setDateMode}
              options={[{ value: 'range', label: 'Range' }, { value: 'on', label: 'Specific day' }]}
            />
          </div>
          {dateMode === 'on' ? (
            <input type="date" aria-label="Day" value={on} onChange={e => setOn(e.target.value)} className="nv-input nv-input--sm" />
          ) : (
            <div className="grid grid-cols-2 gap-2">
              <label>
                <span className="nv-field-label">From</span>
                <input type="date" value={from} onChange={e => setFrom(e.target.value)} className="nv-input nv-input--sm" />
              </label>
              <label>
                <span className="nv-field-label">To</span>
                <input type="date" value={to} onChange={e => setTo(e.target.value)} className="nv-input nv-input--sm" />
              </label>
            </div>
          )}
          <div>
            <span className="nv-field-label">Order</span>
            <Segmented
              block
              label="Sort order"
              value={sortAsc ? 'asc' : 'desc'}
              onChange={v => setSortAsc(v === 'asc')}
              options={[{ value: 'desc', label: 'Newest first' }, { value: 'asc', label: 'Oldest first' }]}
            />
          </div>
          <div className="nv-pop-foot">
            {hasActive && <Button size="sm" variant="ghost" onClick={reset}>Reset</Button>}
            <Button size="sm" className="ml-auto" onClick={apply}>Apply</Button>
          </div>
        </div>
      </PopoverPanel>
    </Popover.Root>
  )
}

function CopyButton({ text, onCopiedChange }: { text: string; onCopiedChange: (copied: boolean) => void }) {
  const [copied, setCopied] = useState(false)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => () => { if (timerRef.current) clearTimeout(timerRef.current) }, [])

  const handleCopy = () => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      onCopiedChange(true)
      if (timerRef.current) clearTimeout(timerRef.current)
      timerRef.current = setTimeout(() => { setCopied(false); onCopiedChange(false) }, 1800)
    }).catch(() => toast.error('Could not copy to the clipboard'))
  }

  return (
    <IconButton label={copied ? 'Copied' : 'Copy transcript'} tone={copied ? 'success' : 'accent'} onClick={handleCopy} className={copied ? 'text-success!' : undefined}>
      <AnimatePresence mode="popLayout" initial={false}>
        <motion.span
          key={copied ? 'done' : 'copy'}
          className="flex"
          initial={{ scale: 0.4, opacity: 0 }}
          animate={{ scale: 1, opacity: 1 }}
          exit={{ scale: 0.4, opacity: 0 }}
          transition={{ type: 'spring', stiffness: 600, damping: 30 }}
        >
          {copied ? <Check strokeWidth={2.5} /> : <Copy strokeWidth={1.9} />}
        </motion.span>
      </AnimatePresence>
    </IconButton>
  )
}

function Entry({ item, onDelete }: { item: Transcript; onDelete: () => void }) {
  const [copied, setCopied] = useState(false)
  return (
    <motion.article
      className="nv-entry overflow-hidden"
      initial={{ opacity: 0, y: -4 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, height: 0, paddingTop: 0, paddingBottom: 0 }}
      transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
    >
      <time className="nv-entry__time" dateTime={item.createdAt} title={fmtDate(item.createdAt)}>
        {fmtClock(item.createdAt)}
      </time>
      <p className="nv-entry__text">{item.content}</p>
      <div className="nv-entry__meta">
        {item.targetApp && <span><AppWindow aria-hidden />Pasted in {item.targetApp}</span>}
        <span><Type aria-hidden />{item.wordCount} {item.wordCount === 1 ? 'word' : 'words'}</span>
        {item.durationSeconds != null && <span><Timer aria-hidden />{fmtTime(Math.round(item.durationSeconds))}</span>}
      </div>
      <div className="nv-entry__actions" data-active={copied || undefined}>
        <CopyButton text={item.content} onCopiedChange={setCopied} />
        <IconButton label="Delete transcript" tone="danger" onClick={onDelete}>
          <Trash2 strokeWidth={1.9} />
        </IconButton>
      </div>
    </motion.article>
  )
}

function FeedSkeleton() {
  return (
    <div className="nv-day">
      <div className="nv-skel mb-3 ml-1 h-3.5 w-24" />
      <div className="nv-card nv-log">
        {[0, 1, 2].map(i => (
          <div key={i} className="nv-entry">
            <div className="nv-skel h-3 w-10" />
            <div className="flex flex-col gap-2">
              <div className="nv-skel h-3 w-full" />
              <div className="nv-skel h-3 w-2/3" />
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}

type Day = { day: DayGroup; items: Transcript[] }

export function Dashboard() {
  const [query, setQuery] = useState('')
  const [searchTerm, setSearchTerm] = useState('')
  const [filters, setFilters] = useState<TranscriptFilters>(NO_FILTERS)
  const observerRef = useRef<IntersectionObserver | null>(null)

  const isSearchMode = searchTerm.length > 0
  const feed = useTranscripts(filters, !isSearchMode)
  const search = useTranscriptSearch(searchTerm, filters)
  const active = isSearchMode ? search : feed
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

  // Groups consecutive rows, so it holds for either sort order.
  const days = useMemo(() => {
    const now = new Date()
    const out: Day[] = []
    for (const item of displayItems) {
      const day = dayOf(item.createdAt, now)
      const last = out.at(-1)
      if (last && last.day.key === day.key) last.items.push(item)
      else out.push({ day, items: [item] })
    }
    return out
  }, [displayItems])

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
    <div className="nv-page">
      <Hero />

      <section className="nv-feed" aria-label="Transcripts">
        <StickyBar className="nv-feed__bar">
          <div className="nv-feed__heading">
            <h2>{isSearchMode ? 'Search results' : 'Transcripts'}</h2>
            {!isSearchMode && feedCount > 0 && <span className="nv-count">{feedCount}</span>}
          </div>
          <div className="nv-feed__tools">
            <SearchInput
              value={query}
              onChange={e => setQuery(e.target.value)}
              placeholder="Search transcripts…"
              aria-label="Search transcripts"
            />
            <FilterButton filters={filters} onChange={setFilters} />
            <ExportButton />
          </div>
        </StickyBar>

        <SectionState status={active.status} error={active.error?.message} onRetry={active.refetch} skeleton={<FeedSkeleton />}>
          {displayItems.length === 0 ? (
            <div className="nv-card nv-empty mt-4">
              <span className="nv-mark nv-mark--lg nv-mark--accent nv-empty__mark">
                {isSearchMode ? <Search size={20} strokeWidth={1.8} /> : <Mic size={20} strokeWidth={1.8} />}
              </span>
              <p className="nv-empty__title">{isSearchMode ? 'No results found' : 'Nothing here yet'}</p>
              <p className="nv-empty__desc">
                {isSearchMode
                  ? 'Try different words, or check the spelling.'
                  : 'Hold your hotkey and speak. Every transcript lands here, newest first.'}
              </p>
            </div>
          ) : (
            <>
              {days.map(({ day, items }) => (
                <section key={day.key} className="nv-day" aria-label={day.label}>
                  <h3 className="nv-day__label">
                    {day.label}
                    {day.detail && <span className="nv-day__date">{day.detail}</span>}
                  </h3>
                  <div className="nv-card nv-log">
                    <AnimatePresence initial={false}>
                      {items.map(item => (
                        <Entry key={item.id} item={item} onDelete={() => deleteTranscript.mutate(item.id)} />
                      ))}
                    </AnimatePresence>
                  </div>
                </section>
              ))}

              {/* Infinite scroll sentinel — pages the feed and search alike. */}
              {hasNextPage && (
                <div ref={sentinelRef} className="grid place-items-center py-6">
                  <span className="nv-ring size-5!" />
                </div>
              )}
            </>
          )}
        </SectionState>
      </section>
    </div>
  )
}
