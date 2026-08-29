import { useState, useRef, useEffect, useCallback } from 'react'
import { useNavigate } from 'react-router'
import { AnimatePresence, motion } from 'framer-motion'
import { Popover } from 'radix-ui'
import {
  Hash, Timer, Mic, Activity,
  AlertCircle, Copy, Check, Trash2,
  Settings2, Search, Download, SlidersHorizontal, LayoutDashboard,
} from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from '../lib/commands'
import { toast } from 'sonner'
import { useAppStore } from '../store/useAppStore'
import { ROUTES } from '../lib/routes'
import { fmtTime, fmtDate, downloadBlob } from '../lib/utils'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { SectionState } from '../components/SectionState'
import type { Transcript } from '../store/useAppStore'

function StatsSkeleton() {
  return (
    <div className="grid grid-cols-4 gap-2.5">
      {[0, 1, 2, 3].map(i => (
        <div key={i} className="flex items-center gap-3.5 px-4.5 py-4 rounded-(--r-xl) bg-(--panel) border border-(--border)">
          <div className="w-9 h-9 rounded-(--r-md) bg-(--surface) animate-pulse shrink-0" />
          <div className="flex flex-col gap-1.5">
            <div className="h-4 w-14 rounded bg-(--surface) animate-pulse" />
            <div className="h-2.5 w-16 rounded bg-(--surface) animate-pulse" />
          </div>
        </div>
      ))}
    </div>
  )
}

function FeedSkeleton() {
  return (
    <div className="flex flex-col gap-3 pr-1.5">
      {[0, 1, 2, 3].map(i => (
        <div key={i} className="grid grid-cols-[20px_1fr] gap-x-3.5">
          <div className="w-2 h-2 rounded-full bg-(--surface) animate-pulse mt-3 justify-self-center" />
          <div className="bg-(--panel) border border-(--border-soft) rounded-(--r-lg) px-3.5 py-3 flex flex-col gap-2">
            <div className="h-3 w-full rounded bg-(--surface) animate-pulse" />
            <div className="h-3 w-3/4 rounded bg-(--surface) animate-pulse" />
          </div>
        </div>
      ))}
    </div>
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
    } finally {
      setExporting(false)
    }
  }

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button
          type="button"
          disabled={exporting}
          title="Export transcripts"
          className="nv-edge inline-flex items-center gap-1.25 h-7 px-2.5 rounded-(--r-md) bg-(--panel) text-[11px] font-medium text-(--fg-2) hover:text-(--fg) hover:[--edge:color-mix(in_srgb,var(--accent)_60%,transparent)] cursor-pointer disabled:opacity-50 shrink-0"
        >
          <Download size={11} strokeWidth={2} />
          Export
        </button>
      </Popover.Trigger>
      <AnimatePresence>
        {open && (
          <Popover.Portal forceMount>
            <Popover.Content align="end" sideOffset={4} asChild>
              <motion.div
                initial={{ opacity: 0, y: -4, scale: 0.97 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: -4, scale: 0.97 }}
                transition={{ duration: 0.14, ease: 'easeOut' }}
                className="z-50 flex flex-col rounded-(--r-lg) border border-(--border) bg-(--panel) shadow-(--shadow-md) overflow-hidden min-w-37 origin-top-right"
              >
                {(['txt', 'json'] as const).map(fmt => (
                  <button
                    key={fmt}
                    type="button"
                    onClick={() => doExport(fmt)}
                    className="px-3 py-1.75 text-left text-[12px] text-(--fg-2) hover:bg-accent hover:text-(--fg) transition-colors cursor-pointer bg-transparent border-none"
                  >
                    {fmt === 'txt' ? 'Plain text (.txt)' : 'JSON (.json)'}
                  </button>
                ))}
              </motion.div>
            </Popover.Content>
          </Popover.Portal>
        )}
      </AnimatePresence>
    </Popover.Root>
  )
}

const STATS = [
  { key: 'totalWords',          label: 'Total Words',   fmt: (v: number) => v.toLocaleString(),   Icon: Hash },
  { key: 'speakingTimeSeconds', label: 'Speaking Time', fmt: (v: number) => fmtTime(v),           Icon: Timer },
  { key: 'totalSessions',       label: 'Sessions',      fmt: (v: number) => v.toLocaleString(),   Icon: Mic },
  { key: 'avgPaceWpm',          label: 'Avg Pace',      fmt: (v: number) => `${v}`,               Icon: Activity },
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
    }).catch(() => {})
  }
  return (
    <button
      type="button"
      className={`inline-flex items-center gap-1 bg-transparent border-none cursor-pointer text-[10px] font-medium px-1.5 py-0.5 rounded-(--r-sm) tracking-[0.02em] transition-colors duration-(--t-fast) ${copied ? 'text-(--success)' : 'text-muted-foreground hover:text-(--accent)'}`}
      onClick={handleCopy}
      title="Copy to clipboard"
    >
      {copied ? <Check size={11} strokeWidth={2.5} /> : <Copy size={11} strokeWidth={2} />}
      {copied ? 'Copied' : 'Copy'}
    </button>
  )
}

type DateMode = 'range' | 'on'

function FilterDropdown() {
  const { filterFrom, filterTo, filterSortAsc, setFilters } = useAppStore()
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
      setFilters(on, on, sortAsc)
    } else {
      setFilters(from || null, to || null, sortAsc)
    }
    setOpen(false)
  }
  const reset = () => {
    setFrom(''); setTo(''); setOn(''); setSortAsc(false)
    setFilters(null, null, false)
    setOpen(false)
  }

  return (
    <Popover.Root open={open} onOpenChange={handleOpenChange}>
      <Popover.Trigger asChild>
        <button
          type="button"
          className={`nv-edge inline-flex items-center gap-1.25 h-7 px-2.5 rounded-(--r-md) text-[11px] font-medium cursor-pointer shrink-0 ${hasActive ? '[--edge:color-mix(in_srgb,var(--accent)_60%,transparent)] bg-(--accent-soft) text-(--accent)' : 'bg-(--panel) text-(--fg-2) hover:text-(--fg) hover:[--edge:color-mix(in_srgb,var(--accent)_60%,transparent)]'}`}
        >
          <SlidersHorizontal size={11} strokeWidth={2} />
          Filter{hasActive ? ' ·' : ''}
        </button>
      </Popover.Trigger>
      <AnimatePresence>
        {open && (
          <Popover.Portal forceMount>
            <Popover.Content align="end" sideOffset={6} asChild>
              <motion.div
                initial={{ opacity: 0, y: -4, scale: 0.97 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: -4, scale: 0.97 }}
                transition={{ duration: 0.14, ease: 'easeOut' }}
                className="z-50 rounded-(--r-lg) border border-(--border) bg-(--panel) shadow-(--shadow-lg) p-3 w-70 origin-top-right"
              >
                <div className="flex flex-col gap-3">
                  {/* Date mode toggle */}
                  <div className="flex flex-col gap-1.25">
                    <span className="text-[11px] font-medium text-muted-foreground">Date</span>
                    <div className="flex gap-1">
                      {(['range', 'on'] as const).map(mode => (
                        <button key={mode} type="button" onClick={() => setDateMode(mode)}
                          className={`flex-1 h-6.5 rounded-(--r-sm) text-[11px] font-medium border transition-colors cursor-pointer ${dateMode === mode ? 'border-(--accent) bg-(--accent-soft) text-(--accent)' : 'border-(--border) bg-transparent text-(--fg-2) hover:text-(--fg)'}`}>
                          {mode === 'range' ? 'Range' : 'Specific day'}
                        </button>
                      ))}
                    </div>
                  </div>
                  {/* Date inputs */}
                  {dateMode === 'on' ? (
                    <input type="date" value={on} onChange={e => setOn(e.target.value)}
                      className="nv-input h-7 text-[11px] px-2 w-full" />
                  ) : (
                    <div className="flex gap-2">
                      <div className="flex flex-col gap-1.25 flex-1">
                        <span className="text-[11px] font-medium text-muted-foreground">From</span>
                        <input type="date" value={from} onChange={e => setFrom(e.target.value)}
                          className="nv-input h-7 text-[11px] px-2 w-full" />
                      </div>
                      <div className="flex flex-col gap-1.25 flex-1">
                        <span className="text-[11px] font-medium text-muted-foreground">To</span>
                        <input type="date" value={to} onChange={e => setTo(e.target.value)}
                          className="nv-input h-7 text-[11px] px-2 w-full" />
                      </div>
                    </div>
                  )}
                  {/* Sort */}
                  <div className="flex flex-col gap-1.25">
                    <span className="text-[11px] font-medium text-muted-foreground">Sort order</span>
                    <div className="flex gap-1">
                      {([false, true] as const).map(asc => (
                        <button key={String(asc)} type="button" onClick={() => setSortAsc(asc)}
                          className={`flex-1 h-6.5 rounded-(--r-sm) text-[11px] font-medium border transition-colors cursor-pointer ${sortAsc === asc ? 'border-(--accent) bg-(--accent-soft) text-(--accent)' : 'border-(--border) bg-transparent text-(--fg-2) hover:text-(--fg)'}`}>
                          {asc ? 'Oldest first' : 'Newest first'}
                        </button>
                      ))}
                    </div>
                  </div>
                  {/* Actions */}
                  <div className="flex items-center gap-2 pt-1 border-t border-(--border-soft)">
                    {hasActive && (
                      <button type="button" onClick={reset}
                        className="text-[11px] text-muted-foreground hover:text-(--fg) transition-colors cursor-pointer bg-transparent border-none">
                        Reset
                      </button>
                    )}
                    <button type="button" onClick={apply}
                      className="ml-auto inline-flex items-center h-6.5 px-3 rounded-(--r-sm) bg-(--accent) text-primary-foreground text-[11px] font-semibold cursor-pointer border-none hover:opacity-90 transition-opacity">
                      Apply
                    </button>
                  </div>
                </div>
              </motion.div>
            </Popover.Content>
          </Popover.Portal>
        )}
      </AnimatePresence>
    </Popover.Root>
  )
}

export function Dashboard() {
  const {
    transcripts, transcriptHasMore, searchResults, isSearching, stats, hasHotkey,
    transcriptsStatus, transcriptsError, statsStatus, statsError,
    loadStats, retryTranscripts, searchTranscripts, deleteTranscript,
  } = useAppStore()
  const navigate = useNavigate()
  const [query, setQuery] = useState('')
  const observerRef = useRef<IntersectionObserver | null>(null)
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const isSearchMode = query.trim().length > 0
  const displayItems = isSearchMode ? searchResults : transcripts

  // A ref callback, not an effect: the sentinel mounts only after the feed's
  // skeleton is replaced, which changes no effect dependency — an effect would
  // read a null ref and never re-run. Reads the action off the store at fire time
  // so the observer isn't rebuilt on every store update.
  const sentinelRef = useCallback((node: HTMLDivElement | null) => {
    observerRef.current?.disconnect()
    if (!node) return
    observerRef.current = new IntersectionObserver(
      (entries) => { if (entries[0].isIntersecting) void useAppStore.getState().loadMoreTranscripts() },
      { threshold: 0.1 }
    )
    observerRef.current.observe(node)
  }, [])

  useEffect(() => () => observerRef.current?.disconnect(), [])

  // Clear search timer on unmount to prevent state updates on unmounted component
  useEffect(() => () => { if (searchTimerRef.current) clearTimeout(searchTimerRef.current) }, [])

  // Debounced search
  const handleSearch = useCallback((value: string) => {
    setQuery(value)
    if (searchTimerRef.current) clearTimeout(searchTimerRef.current)
    searchTimerRef.current = setTimeout(() => searchTranscripts(value), 300)
  }, [searchTranscripts])

  return (
    <div className="flex flex-col h-full overflow-hidden px-8 pt-7 pb-4 gap-7">

      {/* Hero */}
      <div className="flex items-center justify-between gap-4 pb-5 border-b border-(--border-soft)">
        <div className="flex items-center gap-3.5">
          <div className="w-9 h-9 rounded-(--r-lg) bg-(--accent-soft) text-(--accent) flex items-center justify-center shrink-0">
            <LayoutDashboard size={18} strokeWidth={2} />
          </div>
          <div>
            <h1 className="text-[18px] font-bold tracking-tight text-(--fg) leading-[1.1] m-0">Dashboard</h1>
            <p className="text-[12px] text-muted-foreground mt-0.75 m-0">Your voice, transcribed instantly.</p>
          </div>
        </div>
      </div>

      {/* Notices */}
      <AnimatePresence>
        {!hasHotkey && (
          <motion.div key="hotkey-notice" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
            <div className="flex items-center gap-2.5 px-3.5 py-2.5 rounded-(--r-lg) text-[12px] leading-[1.4] shrink-0 text-(--fg-2)" style={{ background: 'var(--warning-soft)', border: '1px solid oklch(from var(--warning) l c h / 0.25)' }}>
              <AlertCircle size={14} strokeWidth={2} className="shrink-0 text-(--warning)" />
              <span className="flex-1">No hotkey set — NexusVoice won't record until you configure one.</span>
              <Button size="sm" onClick={() => navigate(ROUTES.SETTINGS, { state: { tab: 'general' } })} className="shrink-0">
                <Settings2 size={12} strokeWidth={2} />
                Set hotkey
              </Button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Stats */}
      <SectionState status={statsStatus} error={statsError} onRetry={loadStats} skeleton={<StatsSkeleton />}>
      <div className="grid grid-cols-4 gap-2.5">
        {STATS.map(({ key, label, fmt, Icon }, i) => {
          const raw = stats?.[key as keyof typeof stats] as number | undefined
          return (
            <motion.div
              key={key}
              className="flex items-center gap-3.5 px-4.5 py-4 rounded-(--r-xl) bg-(--panel) border border-(--border) cursor-default"
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.22, delay: i * 0.06 }}
            >
              <div className="w-9 h-9 rounded-(--r-md) bg-(--accent-soft) text-(--accent) flex items-center justify-center shrink-0">
                <Icon size={15} strokeWidth={1.75} />
              </div>
              <div className="flex flex-col gap-0.75">
                <span className="text-[20px] font-bold tracking-[-0.03em] text-(--fg) leading-none tabular-nums">{raw != null ? fmt(raw) : '—'}</span>
                <span className="text-[11px] text-muted-foreground font-medium">{label}</span>
              </div>
            </motion.div>
          )
        })}
      </div>
      </SectionState>

      {/* Activity feed */}
      <div className="flex flex-col flex-1 min-h-0 overflow-hidden">
        <div className="flex items-center gap-2.5 mb-4 pr-3">
          <h2 className="text-[13px] font-semibold text-(--fg-2) tracking-[-0.01em] m-0">Recent activity</h2>
          {!isSearchMode && transcripts.length > 0 && (
            <span className="inline-flex items-center justify-center min-w-5 h-5 px-1.5 rounded-full bg-(--accent-soft) text-(--accent) text-[10px] font-bold tracking-[0.02em]">
              {transcripts.length}
            </span>
          )}
          <div className="ml-auto flex items-center gap-2">
          <ExportButton />
          <FilterDropdown />
          {/* Search bar */}
          <div className="relative flex items-center">
            <Search size={12} strokeWidth={2} className="absolute left-2.25 text-muted-foreground pointer-events-none" />
            <Input
              value={query}
              onChange={e => handleSearch(e.target.value)}
              placeholder="Search transcripts…"
              className="pl-7 h-7 text-[12px] w-45"
            />
          </div>
          </div>
        </div>

        <SectionState
          status={transcriptsStatus}
          error={transcriptsError}
          onRetry={retryTranscripts}
          skeleton={<FeedSkeleton />}
        >
        {displayItems.length === 0 && !isSearching ? (
          <div className="flex flex-col items-center gap-3 py-14 px-6 text-center">
            <div className="w-14 h-14 rounded-full border-[1.5px] border-dashed border-(--border) flex items-center justify-center text-muted-foreground">
              {isSearchMode ? <Search size={20} strokeWidth={1.5} /> : <Mic size={20} strokeWidth={1.5} />}
            </div>
            <p className="text-[13px] font-semibold text-(--fg-2) m-0">{isSearchMode ? 'No results found' : 'Nothing here yet'}</p>
            <p className="text-[12px] text-muted-foreground max-w-65 leading-[1.6] m-0">
              {isSearchMode ? 'Try different keywords or check your spelling.' : 'Hold your hotkey and speak — transcripts stream in automatically.'}
            </p>
          </div>
        ) : (
          <div className="flex flex-col gap-0 overflow-y-auto overflow-x-hidden overscroll-none flex-1 min-h-0 pr-1.5">
            <AnimatePresence initial={false}>
              {displayItems.map((item) => (
                <motion.article
                  key={item.id}
                  className="grid grid-cols-[20px_1fr] gap-x-3.5 relative pb-4"
                  initial={{ opacity: 0, y: -6 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.18 }}
                  layout
                >
                  {/* Timeline line */}
                  <div className="absolute left-2.25 top-5.5 -bottom-4 w-px bg-(--border-soft) last:hidden" aria-hidden />
                  {/* Dot */}
                  <div className="col-start-1 row-start-1 w-2 h-2 rounded-full bg-(--accent) mt-3 justify-self-center relative z-10 shrink-0" aria-hidden />
                  {/* Card */}
                  <div className="nv-edge [--edge:var(--border-soft)] hover:[--edge:var(--border)] col-start-2 row-start-1 bg-(--panel) rounded-(--r-lg) px-3.5 py-3 flex flex-col gap-2 hover:bg-(--surface)">
                    <p className="text-[13px] text-(--fg) leading-[1.6] m-0">{item.content}</p>
                    <div className="flex items-center justify-between">
                      <span className="text-[10px] text-muted-foreground tabular-nums">{fmtDate(item.createdAt)}</span>
                      <div className="flex items-center gap-0.5">
                        <CopyButton text={item.content} />
                        <button
                          type="button"
                          className="inline-flex items-center gap-1 bg-transparent border-none cursor-pointer text-[10px] font-medium px-1.5 py-0.5 rounded-(--r-sm) tracking-[0.02em] transition-colors duration-(--t-fast) text-muted-foreground hover:text-destructive"
                          onClick={() => deleteTranscript(item.id)}
                          title="Delete transcript"
                        >
                          <Trash2 size={11} strokeWidth={2} />
                          Delete
                        </button>
                      </div>
                    </div>
                  </div>
                </motion.article>
              ))}
            </AnimatePresence>

            {/* Infinite scroll sentinel — only shown when not searching */}
            {!isSearchMode && transcriptHasMore && (
              <div ref={sentinelRef} className="flex items-center justify-center py-4">
                <motion.div className="w-4 h-4 rounded-full border-2 border-(--border) border-t-(--accent)" animate={{ rotate: 360 }} transition={{ duration: 0.65, ease: 'linear', repeat: Infinity }} />
              </div>
            )}
          </div>
        )}
        </SectionState>
      </div>

    </div>
  )
}
