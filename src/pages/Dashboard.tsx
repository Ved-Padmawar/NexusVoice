import { useState, useRef, useEffect, useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { AnimatePresence, motion } from 'framer-motion'
import {
  Hash, Timer, Mic, Activity,
  AlertCircle, Copy, Check, X,
  Settings2, Zap, Search, Download, SlidersHorizontal,
} from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import type { Transcript } from '../store/useAppStore'

function fmtTime(s: number) {
  if (s < 60) return `${s}s`
  const m = Math.floor(s / 60), rem = s % 60
  if (m < 60) return rem > 0 ? `${m}m ${rem}s` : `${m}m`
  const h = Math.floor(m / 60), rm = m % 60
  return rm > 0 ? `${h}h ${rm}m` : `${h}h`
}

function fmtDate(d: string) {
  try {
    return new Date(d).toLocaleString(undefined, {
      month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
    })
  } catch { return d }
}

function downloadBlob(content: string, filename: string, mime: string) {
  const blob = new Blob([content], { type: mime })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

function ExportButton() {
  const [open, setOpen] = useState(false)
  const [exporting, setExporting] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [open])

  const doExport = async (format: 'txt' | 'json') => {
    setOpen(false)
    setExporting(true)
    try {
      const items = await invoke<Transcript[]>('export_transcripts')
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
    <div ref={ref} className="relative flex-shrink-0">
      <button
        type="button"
        onClick={() => setOpen(v => !v)}
        disabled={exporting}
        title="Export transcripts"
        className="inline-flex items-center gap-[5px] h-[28px] px-[10px] rounded-[var(--r-md)] border border-[var(--border)] bg-[var(--panel)] text-[11px] font-medium text-[var(--fg-2)] hover:text-[var(--fg)] hover:border-[var(--accent)] transition-colors duration-[var(--t-fast)] cursor-pointer disabled:opacity-50"
      >
        <Download size={11} strokeWidth={2} />
        Export
      </button>
      {open && (
        <div className="absolute right-0 top-[calc(100%+4px)] z-50 flex flex-col rounded-[var(--r-lg)] border border-[var(--border)] bg-[var(--panel)] shadow-[var(--shadow-md)] overflow-hidden min-w-[148px]">
          {(['txt', 'json'] as const).map(fmt => (
            <button
              key={fmt}
              type="button"
              onClick={() => doExport(fmt)}
              className="px-3 py-[7px] text-left text-[12px] text-[var(--fg-2)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)] transition-colors cursor-pointer bg-transparent border-none"
            >
              {fmt === 'txt' ? 'Plain text (.txt)' : 'JSON (.json)'}
            </button>
          ))}
        </div>
      )}
    </div>
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
  const handleCopy = () => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }).catch(() => {})
  }
  return (
    <button
      type="button"
      className={`inline-flex items-center gap-1 bg-transparent border-none cursor-pointer text-[10px] font-medium px-[6px] py-[2px] rounded-[var(--r-sm)] tracking-[0.02em] transition-colors duration-[var(--t-fast)] ${copied ? 'text-[var(--success)]' : 'text-[var(--muted)] hover:text-[var(--accent)]'}`}
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
  const ref = useRef<HTMLDivElement>(null)
  const hasActive = !!filterFrom || !!filterTo || filterSortAsc

  useEffect(() => {
    if (!open) return
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [open])

  const openDropdown = () => {
    setFrom(filterFrom ?? '')
    setTo(filterTo ?? '')
    setSortAsc(filterSortAsc)
    setOn('')
    setOpen(true)
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
    <div ref={ref} className="relative flex-shrink-0">
      <button
        type="button"
        onClick={() => open ? setOpen(false) : openDropdown()}
        className={`inline-flex items-center gap-[5px] h-[28px] px-[10px] rounded-[var(--r-md)] border text-[11px] font-medium transition-colors duration-[var(--t-fast)] cursor-pointer ${hasActive ? 'border-[var(--accent)] bg-[var(--accent-soft)] text-[var(--accent)]' : 'border-[var(--border)] bg-[var(--panel)] text-[var(--fg-2)] hover:text-[var(--fg)] hover:border-[var(--accent)]'}`}
      >
        <SlidersHorizontal size={11} strokeWidth={2} />
        Filter{hasActive ? ' ·' : ''}
      </button>
      {open && (
        <div className="absolute right-0 top-[calc(100%+6px)] z-50 rounded-[var(--r-lg)] border border-[var(--border)] bg-[var(--panel)] shadow-[var(--shadow-lg)] p-3 w-[280px]">
          <div className="flex flex-col gap-3">
            {/* Date mode toggle */}
            <div className="flex flex-col gap-[5px]">
              <span className="text-[10px] font-semibold text-[var(--muted)] uppercase tracking-[0.06em]">Date</span>
              <div className="flex gap-1">
                {(['range', 'on'] as const).map(mode => (
                  <button key={mode} type="button" onClick={() => setDateMode(mode)}
                    className={`flex-1 h-[26px] rounded-[var(--r-sm)] text-[11px] font-medium border transition-colors cursor-pointer ${dateMode === mode ? 'border-[var(--accent)] bg-[var(--accent-soft)] text-[var(--accent)]' : 'border-[var(--border)] bg-transparent text-[var(--fg-2)] hover:text-[var(--fg)]'}`}>
                    {mode === 'range' ? 'Range' : 'Specific day'}
                  </button>
                ))}
              </div>
            </div>
            {/* Date inputs */}
            {dateMode === 'on' ? (
              <input type="date" value={on} onChange={e => setOn(e.target.value)}
                className="nv-input h-[28px] text-[11px] px-2 w-full" />
            ) : (
              <div className="flex gap-2">
                <div className="flex flex-col gap-[5px] flex-1">
                  <span className="text-[10px] font-semibold text-[var(--muted)] uppercase tracking-[0.06em]">From</span>
                  <input type="date" value={from} onChange={e => setFrom(e.target.value)}
                    className="nv-input h-[28px] text-[11px] px-2 w-full" />
                </div>
                <div className="flex flex-col gap-[5px] flex-1">
                  <span className="text-[10px] font-semibold text-[var(--muted)] uppercase tracking-[0.06em]">To</span>
                  <input type="date" value={to} onChange={e => setTo(e.target.value)}
                    className="nv-input h-[28px] text-[11px] px-2 w-full" />
                </div>
              </div>
            )}
            {/* Sort */}
            <div className="flex flex-col gap-[5px]">
              <span className="text-[10px] font-semibold text-[var(--muted)] uppercase tracking-[0.06em]">Sort order</span>
              <div className="flex gap-1">
                {([false, true] as const).map(asc => (
                  <button key={String(asc)} type="button" onClick={() => setSortAsc(asc)}
                    className={`flex-1 h-[26px] rounded-[var(--r-sm)] text-[11px] font-medium border transition-colors cursor-pointer ${sortAsc === asc ? 'border-[var(--accent)] bg-[var(--accent-soft)] text-[var(--accent)]' : 'border-[var(--border)] bg-transparent text-[var(--fg-2)] hover:text-[var(--fg)]'}`}>
                    {asc ? 'Oldest first' : 'Newest first'}
                  </button>
                ))}
              </div>
            </div>
            {/* Actions */}
            <div className="flex items-center gap-2 pt-1 border-t border-[var(--border-soft)]">
              {hasActive && (
                <button type="button" onClick={reset}
                  className="text-[11px] text-[var(--muted)] hover:text-[var(--fg)] transition-colors cursor-pointer bg-transparent border-none">
                  Reset
                </button>
              )}
              <button type="button" onClick={apply}
                className="ml-auto inline-flex items-center h-[26px] px-3 rounded-[var(--r-sm)] bg-[var(--accent)] text-[var(--accent-fg)] text-[11px] font-semibold cursor-pointer border-none hover:opacity-90 transition-opacity">
                Apply
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

export function Dashboard() {
  const { transcripts, transcriptHasMore, searchResults, isSearching, stats, hasHotkey, error, setError, loadMoreTranscripts, searchTranscripts } = useAppStore()
  const navigate = useNavigate()
  const [query, setQuery] = useState('')
  const sentinelRef = useRef<HTMLDivElement>(null)
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Infinite scroll via IntersectionObserver
  useEffect(() => {
    if (!sentinelRef.current) return
    const observer = new IntersectionObserver(
      (entries) => { if (entries[0].isIntersecting) loadMoreTranscripts() },
      { threshold: 0.1 }
    )
    observer.observe(sentinelRef.current)
    return () => observer.disconnect()
  }, [loadMoreTranscripts])

  // Debounced search
  const handleSearch = useCallback((value: string) => {
    setQuery(value)
    if (searchTimerRef.current) clearTimeout(searchTimerRef.current)
    searchTimerRef.current = setTimeout(() => searchTranscripts(value), 300)
  }, [searchTranscripts])

  const isSearchMode = query.trim().length > 0
  const displayItems = isSearchMode ? searchResults : transcripts

  return (
    <div className="flex flex-col h-full overflow-hidden px-8 pt-7 pb-10 gap-7">

      {/* Hero */}
      <div className="flex items-center justify-between gap-4 pb-5 border-b border-[var(--border-soft)]">
        <div className="flex items-center gap-[14px]">
          <div className="w-9 h-9 rounded-[var(--r-lg)] bg-[var(--accent-soft)] text-[var(--accent)] flex items-center justify-center flex-shrink-0">
            <Zap size={18} strokeWidth={2.5} />
          </div>
          <div>
            <h1 className="text-[18px] font-bold tracking-[-0.025em] text-[var(--fg)] leading-[1.1] m-0">Dashboard</h1>
            <p className="text-[12px] text-[var(--muted)] mt-[3px] m-0">Your voice, transcribed instantly.</p>
          </div>
        </div>
      </div>

      {/* Notices */}
      <AnimatePresence>
        {!hasHotkey && (
          <motion.div key="hotkey-notice" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
            <div className="flex items-center gap-[10px] px-[14px] py-[10px] rounded-[var(--r-lg)] text-[12px] leading-[1.4] flex-shrink-0 text-[var(--fg-2)]" style={{ background: 'var(--warning-soft)', border: '1px solid oklch(from var(--warning) l c h / 0.25)' }}>
              <AlertCircle size={14} strokeWidth={2} className="flex-shrink-0 text-[var(--warning)]" />
              <span className="flex-1">No hotkey set — NexusVoice won't record until you configure one.</span>
              <Button size="sm" onClick={() => navigate('/settings', { state: { tab: 'general' } })} className="flex-shrink-0">
                <Settings2 size={12} strokeWidth={2} />
                Set hotkey
              </Button>
            </div>
          </motion.div>
        )}
        {error && (
          <motion.div key="error-notice" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
            <div className="flex items-center gap-[10px] px-[14px] py-[10px] rounded-[var(--r-lg)] text-[12px] leading-[1.4] flex-shrink-0 text-[var(--fg-2)]" style={{ background: 'var(--danger-soft)', border: '1px solid oklch(from var(--danger) l c h / 0.30)' }}>
              <AlertCircle size={14} strokeWidth={2} className="flex-shrink-0 text-[var(--danger)]" />
              <span className="flex-1">{error}</span>
              <button type="button" className="ml-auto text-[var(--muted)] bg-transparent border-none cursor-pointer px-[2px] text-[15px] leading-none rounded-[var(--r-xs)] flex-shrink-0 opacity-60 hover:opacity-100 transition-opacity" onClick={() => setError(null)}>
                <X size={14} strokeWidth={2} />
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Stats */}
      <div className="grid grid-cols-4 gap-[10px]">
        {STATS.map(({ key, label, fmt, Icon }, i) => {
          const raw = stats?.[key as keyof typeof stats] as number | undefined
          return (
            <motion.div
              key={key}
              className="flex items-center gap-[14px] px-[18px] py-4 rounded-[var(--r-xl)] bg-[var(--panel)] border border-[var(--border)] cursor-default"
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.22, delay: i * 0.06 }}
            >
              <div className="w-9 h-9 rounded-[var(--r-md)] bg-[var(--accent-soft)] text-[var(--accent)] flex items-center justify-center flex-shrink-0">
                <Icon size={15} strokeWidth={1.75} />
              </div>
              <div className="flex flex-col gap-[3px]">
                <span className="text-[20px] font-bold tracking-[-0.03em] text-[var(--fg)] leading-none tabular-nums">{raw != null ? fmt(raw) : '—'}</span>
                <span className="text-[11px] text-[var(--muted)] font-medium">{label}</span>
              </div>
            </motion.div>
          )
        })}
      </div>

      {/* Activity feed */}
      <div className="flex flex-col flex-1 min-h-0 overflow-hidden">
        <div className="flex items-center gap-[10px] mb-4 pr-[12px]">
          <h2 className="text-[13px] font-bold text-[var(--fg)] tracking-[-0.01em] m-0">Recent Activity</h2>
          {!isSearchMode && transcripts.length > 0 && (
            <span className="inline-flex items-center justify-center min-w-5 h-5 px-[6px] rounded-full bg-[var(--accent-soft)] text-[var(--accent)] text-[10px] font-bold tracking-[0.02em]">
              {transcripts.length}
            </span>
          )}
          <div className="ml-auto flex items-center gap-2">
          <ExportButton />
          <FilterDropdown />
          {/* Search bar */}
          <div className="relative flex items-center">
            <Search size={12} strokeWidth={2} className="absolute left-[9px] text-[var(--muted)] pointer-events-none" />
            <Input
              value={query}
              onChange={e => handleSearch(e.target.value)}
              placeholder="Search transcripts…"
              className="pl-7 h-[28px] text-[12px] w-[180px]"
            />
          </div>
          </div>
        </div>

        {displayItems.length === 0 && !isSearching ? (
          <div className="flex flex-col items-center gap-3 py-14 px-6 text-center">
            <div className="w-14 h-14 rounded-full border-[1.5px] border-dashed border-[var(--border)] flex items-center justify-center text-[var(--muted)]">
              {isSearchMode ? <Search size={20} strokeWidth={1.5} /> : <Mic size={20} strokeWidth={1.5} />}
            </div>
            <p className="text-[13px] font-semibold text-[var(--fg-2)] m-0">{isSearchMode ? 'No results found' : 'Nothing here yet'}</p>
            <p className="text-[12px] text-[var(--muted)] max-w-[260px] leading-[1.6] m-0">
              {isSearchMode ? 'Try different keywords or check your spelling.' : 'Hold your hotkey and speak — transcripts stream in automatically.'}
            </p>
          </div>
        ) : (
          <div className="flex flex-col gap-0 overflow-y-auto overflow-x-hidden flex-1 min-h-0 pr-[6px]">
            <AnimatePresence initial={false}>
              {displayItems.map((item) => (
                <motion.article
                  key={item.id}
                  className="grid grid-cols-[20px_1fr] gap-x-[14px] relative pb-4"
                  initial={{ opacity: 0, y: -6 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.18 }}
                  layout
                >
                  {/* Timeline line */}
                  <div className="absolute left-[9px] top-[22px] bottom-[-16px] w-px bg-[var(--border-soft)] last:hidden" aria-hidden />
                  {/* Dot */}
                  <div className="col-start-1 row-start-1 w-2 h-2 rounded-full bg-[var(--accent)] mt-3 justify-self-center relative z-10 flex-shrink-0" aria-hidden />
                  {/* Card */}
                  <div className="col-start-2 row-start-1 bg-[var(--panel)] border border-[var(--border-soft)] rounded-[var(--r-lg)] px-[14px] py-3 flex flex-col gap-2 transition-[border-color,background] duration-[var(--t-fast)] hover:border-[var(--border)] hover:bg-[var(--surface)]">
                    <p className="text-[13px] text-[var(--fg)] leading-[1.6] m-0">{item.content}</p>
                    <div className="flex items-center justify-between">
                      <span className="text-[10px] text-[var(--muted)] tabular-nums">{fmtDate(item.createdAt)}</span>
                      <CopyButton text={item.content} />
                    </div>
                  </div>
                </motion.article>
              ))}
            </AnimatePresence>

            {/* Infinite scroll sentinel — only shown when not searching */}
            {!isSearchMode && transcriptHasMore && (
              <div ref={sentinelRef} className="flex items-center justify-center py-4">
                <div className="w-4 h-4 rounded-full border-2 border-[var(--border)] border-t-[var(--accent)] animate-[spin_0.65s_linear_infinite]" />
              </div>
            )}
          </div>
        )}
      </div>

    </div>
  )
}
