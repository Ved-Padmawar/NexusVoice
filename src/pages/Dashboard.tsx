import { memo, useState, useRef, useEffect, useCallback, useMemo, type CSSProperties } from 'react'
import { useNavigate } from 'react-router'
import {
  ArrowCounterClockwiseIcon, BracketsCurlyIcon, CalendarBlankIcon, CaretLeftIcon, CaretRightIcon, CheckIcon,
  CopyIcon, ExportIcon, FileTextIcon, FunnelSimpleIcon, KeyboardIcon, MagnifyingGlassIcon, MicrophoneIcon,
  TrashIcon, WarningCircleIcon,
} from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands } from '../bindings'
import { extractErrorMessage } from '../lib/errors'
import { useAppStore } from '../store/useAppStore'
import {
  useDailyWords,
  useTranscripts,
  useTranscriptSearch,
  useStats,
  useDeleteTranscript,
  fetchAllTranscripts,
  NO_FILTERS,
  type TranscriptFilters,
} from '../lib/queries'
import { ROUTES } from '../lib/routes'
import { cn, fmtTime, fmtDate, downloadBlob, parseSqliteDate } from '../lib/utils'
import { Button } from '@/components/ui/Button'
import { Kbd } from '@/components/ui/Kbd'
import { MenuItem, Popover } from '@/components/ui/Popover'
import { Segmented } from '@/components/ui/Segmented'
import { TextInput } from '@/components/ui/TextInput'
import { SectionState } from '../components/SectionState'
import type { Transcript } from '../types'

const TYPING_WPM = 40

function HeroSkeleton() {
  return <div className="h-64 animate-pulse rounded-2xl bg-panel ring-1 ring-inset ring-line-soft" />
}

function FeedSkeleton() {
  return (
    <div className="flex flex-col gap-5 py-4">
      {[0, 1, 2, 3].map(i => (
        <div key={i} className="grid grid-cols-[84px_1fr] gap-x-5">
          <div className="h-5 w-16 animate-pulse rounded bg-surface" />
          <div className="flex flex-col gap-2">
            <div className="h-3.5 w-full animate-pulse rounded bg-surface" />
            <div className="h-3.5 w-3/4 animate-pulse rounded bg-surface" />
          </div>
        </div>
      ))}
    </div>
  )
}

function IconButton({ className, ...props }: React.ComponentProps<'button'>) {
  return (
    <button
      type="button"
      className={cn(
        'grid size-8 cursor-pointer place-items-center rounded-lg bg-panel text-fg-2 ring-1 ring-inset ring-line-soft transition hover:text-fg hover:ring-line disabled:opacity-50',
        className,
      )}
      {...props}
    />
  )
}

function ExportButton() {
  const [open, setOpen] = useState(false)
  const [exporting, setExporting] = useState(false)
  const anchorRef = useRef<HTMLButtonElement>(null)

  const doExport = async (format: 'txt' | 'json') => {
    setOpen(false)
    setExporting(true)
    try {
      const items = await fetchAllTranscripts()
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
    <>
      <IconButton
        ref={anchorRef}
        aria-label="Export"
        aria-haspopup="menu"
        aria-expanded={open}
        data-tip="Export"
        data-tip-pos="bottom"
        data-tip-align="end"
        disabled={exporting}
        onClick={() => setOpen(o => !o)}
      >
        <ExportIcon size={14} weight="bold" />
      </IconButton>
      <Popover open={open} onOpenChange={setOpen} anchorRef={anchorRef} placement="bottom-end" className="min-w-44">
        <MenuItem onClick={() => doExport('txt')}>
          <FileTextIcon size={16} className="text-muted" />Plain text<span className="ml-auto font-mono text-2xs text-muted">.txt</span>
        </MenuItem>
        <MenuItem onClick={() => doExport('json')}>
          <BracketsCurlyIcon size={16} className="text-muted" />JSON<span className="ml-auto font-mono text-2xs text-muted">.json</span>
        </MenuItem>
      </Popover>
    </>
  )
}

function CopyButton({ text }: { text: string }) {
  const handleCopy = () => {
    navigator.clipboard.writeText(text).catch(() => toast.error('Could not copy to the clipboard'))
  }
  return (
    <button
      type="button"
      aria-label="Copy"
      onClick={handleCopy}
      className="copy-btn grid size-8 cursor-pointer place-items-center rounded-lg text-muted hover:bg-surface-hover hover:text-fg"
    >
      <CopyIcon size={15} className="c-idle" />
      <CheckIcon size={15} weight="bold" className="c-done" />
    </button>
  )
}

type DateMode = 'range' | 'on'
type DateKey = 'from' | 'to' | 'on'
type Dates = Record<DateKey, string>

const pad = (n: number) => String(n).padStart(2, '0')
const toIso = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
const fromIso = (s: string) => { const [y, m, d] = s.split('-').map(Number); return new Date(y, m - 1, d) }
const dateFmt = new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric', year: 'numeric' })
const monthFmt = new Intl.DateTimeFormat('en-US', { month: 'long', year: 'numeric' })
const WEEKDAYS = ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa']

const calFootButton = 'h-6.5 cursor-pointer rounded-[7px] px-2 text-xs font-semibold text-fg-2 transition-colors hover:bg-surface-hover hover:text-fg'

type CalendarProps = {
  view: Date
  onView: (view: Date) => void
  picked: string
  /** A band between the range's ends, only while choosing From or To. */
  band: { from: string; to: string } | null
  onPick: (value: string) => void
}

function Calendar({ view, onView, picked, band, onPick }: CalendarProps) {
  const y = view.getFullYear()
  const m = view.getMonth()
  const lead = new Date(y, m, 1).getDay()
  const today = toIso(new Date())
  const shift = (by: number) => onView(new Date(y, m + by, 1))
  const navButton = 'grid size-7 cursor-pointer place-items-center rounded-[7px] text-fg-2 transition-colors hover:bg-surface-hover hover:text-fg'

  return (
    <>
      <div className="mb-2 flex items-center justify-between">
        <button type="button" aria-label="Previous month" className={navButton} onClick={() => shift(-1)}><CaretLeftIcon size={14} weight="bold" /></button>
        <b className="text-sm font-semibold">{monthFmt.format(view)}</b>
        <button type="button" aria-label="Next month" className={navButton} onClick={() => shift(1)}><CaretRightIcon size={14} weight="bold" /></button>
      </div>
      <div className="grid grid-cols-7 gap-0.5">
        {WEEKDAYS.map(d => (
          <span key={d} className="grid h-6 place-items-center text-2xs font-semibold tracking-wider text-muted uppercase">{d}</span>
        ))}
        {Array.from({ length: 42 }, (_, i) => {
          const d = new Date(y, m, 1 - lead + i)
          const v = toIso(d)
          return (
            <button
              key={v}
              type="button"
              onClick={() => onPick(v)}
              className={cn(
                'grid h-7.5 cursor-pointer place-items-center rounded-[7px] text-xs text-fg transition-colors hover:bg-surface-hover',
                d.getMonth() !== m && 'text-muted',
                v === today && 'ring-1 ring-inset ring-line',
                band && v > band.from && v < band.to && 'rounded-none bg-surface',
                v === picked && 'rounded-[7px] bg-accent-soft font-semibold text-accent-ink ring-1 ring-inset ring-accent-line',
              )}
            >
              {d.getDate()}
            </button>
          )
        })}
      </div>
      <div className="mt-2 flex justify-between border-t border-line-soft pt-2">
        <button type="button" className={calFootButton} onClick={() => onPick('')}>Clear</button>
        <button type="button" className={cn(calFootButton, 'text-accent-ink')} onClick={() => onPick(toIso(new Date()))}>Today</button>
      </div>
    </>
  )
}

function DateFieldButton({ value, expanded, onClick, ref }: {
  value: string
  expanded: boolean
  onClick: () => void
  ref: React.Ref<HTMLButtonElement>
}) {
  return (
    <button
      ref={ref}
      type="button"
      aria-expanded={expanded}
      onClick={onClick}
      className={cn(
        'flex h-8 w-full cursor-pointer items-center gap-2 rounded-lg bg-field px-2.5 text-left text-xs ring-1 ring-inset ring-line-soft transition-shadow hover:ring-line',
        expanded && 'ring-accent hover:ring-accent',
      )}
    >
      <CalendarBlankIcon size={14} className="shrink-0 text-muted" />
      <span className={cn('min-w-0 truncate', value ? 'text-fg' : 'text-muted')}>{value ? dateFmt.format(fromIso(value)) : 'Pick a date'}</span>
    </button>
  )
}

const fieldLabel = 'text-xs font-semibold text-fg-2'

type FilterDropdownProps = {
  filters: TranscriptFilters
  onChange: (filters: TranscriptFilters) => void
}

function FilterDropdown({ filters, onChange }: FilterDropdownProps) {
  const { from: filterFrom, to: filterTo, sortAsc: filterSortAsc } = filters
  const [open, setOpen] = useState(false)
  const [dateMode, setDateMode] = useState<DateMode>('range')
  const [dates, setDates] = useState<Dates>({ from: filterFrom ?? '', to: filterTo ?? '', on: '' })
  const [sortAsc, setSortAsc] = useState(filterSortAsc)
  const [calField, setCalField] = useState<DateKey | null>(null)
  const [calView, setCalView] = useState(() => new Date())
  const hasActive = !!filterFrom || !!filterTo || filterSortAsc

  const anchorRef = useRef<HTMLButtonElement>(null)
  const calRef = useRef<HTMLDivElement>(null)
  const fieldRefs = { from: useRef<HTMLButtonElement>(null), to: useRef<HTMLButtonElement>(null), on: useRef<HTMLButtonElement>(null) }
  const calInside = useMemo(() => [calRef], [])

  const handleOpenChange = (next: boolean) => {
    if (next) {
      setDates({ from: filterFrom ?? '', to: filterTo ?? '', on: '' })
      setSortAsc(filterSortAsc)
    }
    setCalField(null)
    setOpen(next)
  }

  const toggleField = (key: DateKey) => {
    if (calField === key) { setCalField(null); return }
    const value = dates[key]
    const v = value ? fromIso(value) : new Date()
    setCalView(new Date(v.getFullYear(), v.getMonth(), 1))
    setCalField(key)
  }

  const pickDate = (value: string) => {
    if (!calField) return
    const next = { ...dates, [calField]: value }
    // Keep the range ordered: moving one end past the other drops the other.
    if (calField !== 'on' && next.from && next.to && next.from > next.to) {
      if (calField === 'from') next.to = ''
      else next.from = ''
    }
    setDates(next)
    setCalField(null)
  }

  const apply = () => {
    if (dateMode === 'on' && dates.on) {
      onChange({ from: dates.on, to: dates.on, sortAsc })
    } else {
      onChange({ from: dates.from || null, to: dates.to || null, sortAsc })
    }
    handleOpenChange(false)
  }
  const reset = () => {
    setDates({ from: '', to: '', on: '' })
    setSortAsc(false)
    onChange(NO_FILTERS)
    handleOpenChange(false)
  }

  const field = (key: DateKey) => (
    <DateFieldButton ref={fieldRefs[key]} value={dates[key]} expanded={calField === key} onClick={() => toggleField(key)} />
  )

  return (
    <>
      <IconButton
        ref={anchorRef}
        aria-label="Filter"
        aria-haspopup="dialog"
        aria-expanded={open}
        data-tip="Filter"
        data-tip-pos="bottom"
        data-tip-align="end"
        onClick={() => handleOpenChange(!open)}
        className={hasActive ? 'bg-accent-soft text-accent-ink ring-accent-line hover:text-accent-ink hover:ring-accent' : undefined}
      >
        <FunnelSimpleIcon size={14} weight="bold" />
      </IconButton>
      <Popover open={open} onOpenChange={handleOpenChange} anchorRef={anchorRef} placement="bottom-end" insideRefs={calInside} className="w-72 rounded-2xl p-4">
        <div className="flex flex-col gap-3.5">
          <div className="flex flex-col gap-1.5">
            <span className={fieldLabel}>Date</span>
            <Segmented
              size="sm"
              fill
              value={dateMode}
              onChange={setDateMode}
              options={[{ value: 'range', label: 'Range' }, { value: 'on', label: 'One day' }]}
            />
          </div>
          {dateMode === 'on' ? (
            <div className="flex flex-col gap-1.5"><span className={fieldLabel}>On</span>{field('on')}</div>
          ) : (
            <div className="flex gap-2">
              <div className="flex min-w-0 flex-1 flex-col gap-1.5"><span className={fieldLabel}>From</span>{field('from')}</div>
              <div className="flex min-w-0 flex-1 flex-col gap-1.5"><span className={fieldLabel}>To</span>{field('to')}</div>
            </div>
          )}
          <div className="flex flex-col gap-1.5">
            <span className={fieldLabel}>Order</span>
            <Segmented
              size="sm"
              fill
              value={sortAsc ? 'asc' : 'desc'}
              onChange={v => setSortAsc(v === 'asc')}
              options={[{ value: 'desc', label: 'Newest first' }, { value: 'asc', label: 'Oldest first' }]}
            />
          </div>
          <div className="flex items-center gap-2 border-t border-line-soft pt-3">
            <Button size="sm" variant="ghost" onClick={reset}><ArrowCounterClockwiseIcon size={12} weight="bold" />Reset</Button>
            <Button size="sm" variant="tinted" onClick={apply} className="ml-auto px-4">Apply</Button>
          </div>
        </div>
      </Popover>
      <Popover
        open={open && calField !== null}
        onOpenChange={() => setCalField(null)}
        anchorRef={fieldRefs[calField ?? 'from']}
        placement={calField === 'to' ? 'bottom-end' : 'bottom-start'}
        className="w-64"
      >
        <div ref={calRef} className="p-3">
          <Calendar
            view={calView}
            onView={setCalView}
            picked={calField ? dates[calField] : ''}
            band={calField !== 'on' && dates.from && dates.to ? { from: dates.from, to: dates.to } : null}
            onPick={pickDate}
          />
        </div>
      </Popover>
    </>
  )
}

/** Owns the typed text, so a keystroke re-renders only this box, not the feed. */
function SearchBox({ onSearch }: { onSearch: (term: string) => void }) {
  const [query, setQuery] = useState('')

  useEffect(() => {
    const id = setTimeout(() => onSearch(query.trim()), 300)
    return () => clearTimeout(id)
  }, [query, onSearch])

  return (
    <TextInput
      type="search"
      value={query}
      onChange={e => setQuery(e.target.value)}
      placeholder="Search transcripts…"
      aria-label="Search transcripts"
      icon={<MagnifyingGlassIcon size={14} />}
      wrapperClassName="w-56"
      className="h-8"
    />
  )
}

const timeFmt = new Intl.DateTimeFormat(undefined, { hour: 'numeric', minute: '2-digit' })
const dayFmt = new Intl.DateTimeFormat(undefined, { weekday: 'long', month: 'short', day: 'numeric' })

function dayLabel(date: Date) {
  const days = Math.round((new Date().setHours(0, 0, 0, 0) - new Date(date).setHours(0, 0, 0, 0)) / 86_400_000)
  if (days === 0) return 'Today'
  if (days === 1) return 'Yesterday'
  return dayFmt.format(date)
}

function splitClock(date: Date) {
  const parts = timeFmt.formatToParts(date)
  const meridiem = parts.find(p => p.type === 'dayPeriod')?.value
  const clock = parts.filter(p => p.type !== 'dayPeriod').map(p => p.value).join('').trim()
  return { clock, meridiem }
}

type DayGroup = { key: string; label: string; items: Transcript[] }

function groupByDay(items: Transcript[]) {
  const groups: DayGroup[] = []
  for (const item of items) {
    const label = dayLabel(parseSqliteDate(item.createdAt))
    const last = groups.at(-1)
    if (last?.label === label) last.items.push(item)
    else groups.push({ key: `${label}-${item.id}`, label, items: [item] })
  }
  return groups
}

const plural = (n: number, word: string) => `${n.toLocaleString()} ${word}${n === 1 ? '' : 's'}`

const TranscriptRow = memo(function TranscriptRow({ item, onDelete }: { item: Transcript; onDelete: (id: number) => void }) {
  const { clock, meridiem } = splitClock(parseSqliteDate(item.createdAt))
  return (
    <li
      className="group relative grid grid-cols-[84px_minmax(0,1fr)_72px] gap-x-5 py-4 not-first:before:absolute not-first:before:top-0 not-first:before:right-0 not-first:before:left-26 not-first:before:h-px not-first:before:bg-line-soft not-first:before:content-['']"
    >
      <p className="text-xl leading-tight font-bold tracking-tight text-fg tabular-nums">
        {clock}{meridiem && <small className="ml-0.5 text-xs font-semibold tracking-normal text-muted">{meridiem}</small>}
      </p>
      <div className="min-w-0">
        <p className="text-[14.5px] leading-relaxed text-fg-2 text-pretty">{item.content}</p>
        <p className="mt-1 text-xs text-muted tabular-nums">
          <span className="font-semibold text-fg-2">{item.targetApp ?? 'Clipboard'}</span>
          {' · '}{plural(item.wordCount, 'word')}
          {item.durationSeconds != null && ` · ${item.durationSeconds}s`}
        </p>
      </div>
      <div className="-mt-1.5 -mr-2 flex justify-self-end gap-0.5 opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100">
        <CopyButton text={item.content} />
        <button
          type="button"
          aria-label="Delete"
          onClick={() => onDelete(item.id)}
          className="grid size-8 cursor-pointer place-items-center rounded-lg text-muted hover:bg-danger-soft hover:text-danger"
        >
          <TrashIcon size={15} />
        </button>
      </div>
    </li>
  )
})

const statLabel = 'text-xs text-muted'
const statValue = 'text-xl font-semibold tracking-tight text-fg tabular-nums'

function HotkeyKeys({ hotkey }: { hotkey: string }) {
  return (
    <span className="inline-flex gap-1 align-middle">
      {hotkey.split('+').map(key => <Kbd key={key}>{key}</Kbd>)}
    </span>
  )
}

const WAVE_DAYS = 60
const waveTipFmt = new Intl.DateTimeFormat(undefined, { weekday: 'short', month: 'short', day: 'numeric' })
const waveAxisFmt = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' })

/** The last 60 days drawn as a voice: one bar per day, scaled to the busiest. */
function DayWave() {
  const { data } = useDailyWords(WAVE_DAYS)
  if (!data) return <div className="h-33.5" />
  const peak = Math.max(1, ...data.map(d => d.words))
  return (
    <div className="px-8 pt-6 pb-5">
      <div className="daywave flex h-20 items-stretch gap-0.75" role="img" aria-label={`Words per day, last ${WAVE_DAYS} days`}>
        {data.map(({ day, words }, i) => (
          <span
            key={day.getTime()}
            data-tip={`${waveTipFmt.format(day)}\n${plural(words, 'word')}`}
            data-tip-align={i < 6 ? 'start' : i >= WAVE_DAYS - 6 ? 'end' : undefined}
            data-today={i === WAVE_DAYS - 1 || undefined}
            style={{ '--v': words / peak, '--i': i } as CSSProperties}
          >
            <i />
          </span>
        ))}
      </div>
      <div className="mt-2 flex justify-between text-2xs text-muted tabular-nums">
        <span>{waveAxisFmt.format(data[0].day)}</span>
        <span>{waveAxisFmt.format(data[WAVE_DAYS / 2].day)}</span>
        <span className="font-semibold text-accent-ink">Today</span>
      </div>
    </div>
  )
}

export function Dashboard() {
  const hasHotkey = useAppStore(s => s.hasHotkey)
  // The keys to show in the hint; the store only knows whether one is set.
  const [hotkey, setHotkey] = useState<string | null>(null)
  useEffect(() => {
    if (!hasHotkey) return
    commands.getHotkeys().then(h => setHotkey(h.pushToTalk)).catch(() => {})
  }, [hasHotkey])
  const keys = hasHotkey ? hotkey : null
  const navigate = useNavigate()
  const [searchTerm, setSearchTerm] = useState('')
  const [filters, setFilters] = useState<TranscriptFilters>(NO_FILTERS)
  const observerRef = useRef<IntersectionObserver | null>(null)

  const isSearchMode = searchTerm.length > 0
  const feed = useTranscripts(filters, !isSearchMode)
  const search = useTranscriptSearch(searchTerm, filters)
  const active = isSearchMode ? search : feed
  const stats = useStats()
  const deleteTranscript = useDeleteTranscript()
  // Both read the same database: when both fail, one error says it once.
  const bothFailed = stats.status === 'error' && stats.data == null && active.status === 'error'
  const retryFeed = bothFailed
    ? () => { void stats.refetch(); void active.refetch() }
    : active.refetch

  const groups = useMemo(() => {
    const rows = active.data?.pages.flatMap(page => page.rows) ?? []
    const seen = new Set<number>()
    return groupByDay(rows.filter(row => !seen.has(row.id) && seen.add(row.id)))
  }, [active.data])

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

  const s = stats.data
  const savedSeconds = s ? Math.max(0, Math.round((s.totalWords / TYPING_WPM) * 60 - s.speakingTimeSeconds)) : 0
  const hours = Math.floor(savedSeconds / 3600)
  const minutes = Math.floor((savedSeconds % 3600) / 60)
  const dash = (v: string | undefined) => v ?? '—'

  return (
    <div data-no-topbar-fade className="page-scroll min-h-0 overflow-y-auto overscroll-none px-7 pb-8 before:block before:h-3 before:content-['']">
      {!hasHotkey && (
        <div className="mb-4 flex items-center gap-3 rounded-xl bg-warning-soft py-2.5 pr-2.5 pl-4 text-sm text-fg ring-1 ring-inset ring-warning/30">
          <WarningCircleIcon size={18} weight="fill" className="shrink-0 text-warning" />
          <span className="flex-1">No hotkey set. NexusVoice can't hear you until a hotkey is set.</span>
          <Button size="sm" variant="primary" onClick={() => navigate(ROUTES.SETTINGS, { state: { tab: 'general' } })}>
            <KeyboardIcon size={14} weight="bold" />Set a hotkey
          </Button>
        </div>
      )}

      {!bothFailed && (
        <SectionState status={stats.status} error={stats.error?.message} onRetry={stats.refetch} skeleton={<HeroSkeleton />} hasData={s != null}>
          <section className="relative isolate overflow-hidden rounded-2xl bg-panel ring-1 ring-inset ring-line-soft">
            <div className="pointer-events-none absolute inset-0 -z-10 bg-[radial-gradient(70%_120%_at_100%_0%,var(--accent-soft),transparent_60%),radial-gradient(50%_90%_at_0%_100%,var(--accent-2-soft),transparent_70%)]" />
            <div className="flex items-stretch justify-between gap-8 px-8 pt-7">
              <div>
                <p className="text-[13px] font-medium text-fg-2">You skipped typing for</p>
                <p
                  className="mt-2 flex items-baseline gap-1 text-display font-bold tracking-tighter text-fg tabular-nums"
                  aria-label={s ? `${hours} hours ${minutes} minutes` : undefined}
                >
                  {s ? (
                    <>
                      <span className="count" style={{ '--to': hours } as CSSProperties} />
                      <span className="mr-3 text-2xl font-semibold tracking-tight text-muted">h</span>
                      <span className="count" style={{ '--to': minutes } as CSSProperties} />
                      <span className="text-2xl font-semibold tracking-tight text-muted">m</span>
                    </>
                  ) : '—'}
                </p>
                {s && (
                  <p className="mt-3 max-w-[46ch] text-sm text-muted text-pretty">
                    {s.totalWords.toLocaleString()} words at a {TYPING_WPM} wpm typing pace, minus the {fmtTime(s.speakingTimeSeconds)} you spent saying them.
                  </p>
                )}
              </div>
              <div className="flex shrink-0 flex-col items-end justify-between gap-5">
              {keys && (
                <p className="flex items-center gap-1.5 text-xs text-muted">Hold <HotkeyKeys hotkey={keys} /> anywhere to dictate</p>
              )}
              <dl className="grid grid-cols-2 gap-x-9 gap-y-4 pb-1">
                <div><dt className={statLabel}>Words</dt><dd className={statValue}>{dash(s?.totalWords.toLocaleString())}</dd></div>
                <div><dt className={statLabel}>Speaking</dt><dd className={statValue}>{dash(s && fmtTime(s.speakingTimeSeconds))}</dd></div>
                <div><dt className={statLabel}>Dictations</dt><dd className={statValue}>{dash(s?.totalSessions.toLocaleString())}</dd></div>
                <div>
                  <dt className={statLabel}>Pace</dt>
                  <dd className={cn(statValue, 'flex items-center gap-2')}>
                    {dash(s && String(s.avgPaceWpm))}
                    {s && (
                      <>
                        <svg viewBox="0 0 20 20" className="size-4 -rotate-90" aria-hidden="true">
                          <circle cx="10" cy="10" r="8" fill="none" stroke="var(--border)" strokeWidth="3" />
                          <circle className="ring-fill" cx="10" cy="10" r="8" fill="none" stroke="var(--accent)" strokeWidth="3" strokeLinecap="round" pathLength="100" strokeDasharray={`${Math.min(100, s.avgPaceWpm / 2)} 100`} />
                        </svg>
                        <span className="text-xs font-medium text-muted">wpm</span>
                      </>
                    )}
                  </dd>
                </div>
              </dl>
              </div>
            </div>
            <DayWave />
          </section>
        </SectionState>
      )}

      <div className="mt-7">
        {/* The scroll box has no top padding (a spacer above stands in): sticky
            pins against the padding edge, which would leave rows showing above. */}
        <div className="sticky top-0 z-10 -mx-2 flex items-center gap-2.5 bg-bg px-2 py-2.5">
          <h2 className="text-md font-semibold tracking-tight text-fg">Recent dictations</h2>
          <div className="ml-auto flex items-center gap-1.5">
            <SearchBox onSearch={setSearchTerm} />
            <FilterDropdown filters={filters} onChange={setFilters} />
            <ExportButton />
          </div>
        </div>

        <SectionState
          status={active.status}
          error={active.error?.message}
          onRetry={retryFeed}
          skeleton={<FeedSkeleton />}
        >
          {groups.length === 0 ? (
            <div className="flex flex-col items-center gap-3 px-6 py-14 text-center">
              <span className="grid size-14 place-items-center rounded-full bg-accent-soft text-accent-ink">
                {isSearchMode ? <MagnifyingGlassIcon size={24} /> : <MicrophoneIcon size={24} weight="duotone" />}
              </span>
              <p className="text-md font-semibold text-fg">{isSearchMode ? 'No results found' : 'Your first dictation lands here'}</p>
              <p className="max-w-64 text-sm text-muted text-balance">
                {isSearchMode
                  ? 'Try different keywords or check your spelling.'
                  : <>Hold {keys ? <HotkeyKeys hotkey={keys} /> : 'your hotkey'} in any text box, speak, and let go.</>}
              </p>
            </div>
          ) : (
            <div className="flex flex-col">
              {groups.map(group => (
                <section key={group.key} className="mt-2">
                  <h3 className="flex items-center gap-2 py-2 text-xs font-semibold text-fg-2">
                    {group.label}
                    <span className="font-normal text-muted">{plural(group.items.length, 'dictation')}</span>
                    <span className="h-px flex-1 bg-line-soft" />
                  </h3>
                  <ol>
                    {group.items.map(item => (
                      <TranscriptRow key={item.id} item={item} onDelete={deleteTranscript.mutate} />
                    ))}
                  </ol>
                </section>
              ))}

              {/* Infinite scroll sentinel — pages the feed and search alike. */}
              {hasNextPage && (
                <div ref={sentinelRef} className="grid place-items-center py-5">
                  <span className="size-4 animate-[spin_0.65s_linear_infinite] rounded-full border-2 border-line border-t-accent" />
                </div>
              )}
            </div>
          )}
        </SectionState>
      </div>
    </div>
  )
}
