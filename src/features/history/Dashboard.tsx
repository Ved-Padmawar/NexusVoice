import { Fragment, memo, useState, useRef, useEffect, useCallback, useMemo, type CSSProperties, type RefObject } from 'react'
import { useNavigate } from 'react-router'
import {
  BracketsCurlyIcon, CheckIcon,
  ArrowsClockwiseIcon, CopyIcon, ExportIcon, FileAudioIcon, FileTextIcon, KeyboardIcon, MagnifyingGlassIcon, MicrophoneIcon,
  PushPinIcon, TrashIcon, WarningCircleIcon, WaveformIcon, XIcon,
} from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands, type ExportFormat, type JobSnapshot } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import {
  useDailyWords,
  useHistoryFeed,
  useStats,
  hiddenForUndo,
  useDeleteTranscript,
  NO_FILTERS,
} from './queries'
import { hasDates, isFiltered, type TranscriptFilters } from '../../shared/queryKeys'
import { useSlice } from '../../app/mirror'
import { ROUTES } from '../../app/routes'
import { cn, fmtTime, parseSqliteDate } from '../../shared/utils'
import { Button } from '@/shared/ui/Button'
import { Kbd } from '@/shared/ui/Kbd'
import { MenuItem, Popover } from '@/shared/ui/Popover'
import { TextInput } from '@/shared/ui/TextInput'
import { SectionState } from '../../shared/ui/SectionState'
import { AudioPlayer } from './AudioPlayer'
import { ImportControls } from './ImportFlow'
import { FilterChip, FilterDropdown, ToolbarButton } from './HistoryFilters'
import { cancelJob, useHasJob, useJob, useJobs } from './jobs'
import { ACTION_BUTTON, PinButton, RetranscribeButton } from './RowActions'
import type { Transcript } from '../../shared/types'

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

/** Exports what the search box and filters ask for, without waiting on the debounce. */
function ExportButton({ typedRef, filters }: { typedRef: RefObject<string>; filters: TranscriptFilters }) {
  const [open, setOpen] = useState(false)
  const [exporting, setExporting] = useState(false)
  const anchorRef = useRef<HTMLButtonElement>(null)

  const doExport = async (format: ExportFormat) => {
    setOpen(false)
    setExporting(true)
    try {
      const exported = await commands.exportTranscripts({
        query: typedRef.current.trim(), ...filters, exclude: hiddenForUndo(), format,
      })
      if (!exported) {
        toast.info('Nothing to export')
        return
      }
      toast.success(`Exported ${plural(exported.count, 'dictation')}`, {
        description: exported.path,
        action: { label: 'Show in folder', onClick: () => void commands.revealInFolder(exported.path).catch(e => toast.error(extractErrorMessage(e, 'Could not open the folder'))) },
      })
    } catch (e) {
      toast.error(extractErrorMessage(e, 'Export failed'))
    } finally {
      setExporting(false)
    }
  }

  return (
    <>
      <ToolbarButton
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
      </ToolbarButton>
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
      className={cn('copy-btn', ACTION_BUTTON)}
    >
      <CopyIcon size={15} className="c-idle" />
      <CheckIcon size={15} weight="bold" className="c-done" />
    </button>
  )
}

/** Owns the typed text, so a keystroke re-renders only this box, not the feed.
 *  `typedRef` mirrors it, undebounced. */
function SearchBox({ onSearch, typedRef }: { onSearch: (term: string) => void; typedRef: RefObject<string> }) {
  const [query, setQuery] = useState('')

  useEffect(() => {
    const id = setTimeout(() => onSearch(query.trim()), 300)
    return () => clearTimeout(id)
  }, [query, onSearch])

  return (
    <TextInput
      type="search"
      value={query}
      onChange={e => {
        typedRef.current = e.target.value
        setQuery(e.target.value)
      }}
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
const dayYearFmt = new Intl.DateTimeFormat(undefined, { weekday: 'long', month: 'short', day: 'numeric', year: 'numeric' })

function dayLabel(date: Date) {
  const days = Math.round((new Date().setHours(0, 0, 0, 0) - new Date(date).setHours(0, 0, 0, 0)) / 86_400_000)
  if (days === 0) return 'Today'
  if (days === 1) return 'Yesterday'
  return (date.getFullYear() === new Date().getFullYear() ? dayFmt : dayYearFmt).format(date)
}

/** The local calendar day, e.g. `2026-11-09`: unique across years, unlike a label. */
function calendarDay(date: Date) {
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
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
    const date = parseSqliteDate(item.createdAt)
    const key = calendarDay(date)
    const last = groups.at(-1)
    if (last?.key === key) last.items.push(item)
    // Keyed by day, not first row, so a new row doesn't remount the day.
    else groups.push({ key, label: dayLabel(date), items: [item] })
  }
  return groups
}

/** The last entry whose audio the limit still keeps, when older ones lost theirs. */
function audioBoundary(rows: Transcript[], keep: number, more: boolean): number | null {
  const kept = rows.filter(row => row.audioPath && !row.pinned)
  if (keep === 0 || kept.length !== keep) return null
  const last = kept[keep - 1]
  const older = rows.slice(rows.indexOf(last) + 1)
  return more || older.some(row => !row.audioPath) ? last.id : null
}

const plural = (n: number, word: string) => `${n.toLocaleString()} ${word}${n === 1 ? '' : 's'}`

/** Where the saved-audio limit falls in the feed. */
function AudioBoundary({ keep, onChange }: { keep: number; onChange: () => void }) {
  return (
    <li className="flex items-center gap-3 py-2 pl-26 text-xs text-muted">
      <span className="h-px w-6 bg-line-soft" />
      <WaveformIcon size={13} />
      <span>Audio is kept for your last {keep === 1 ? 'entry' : `${keep} entries`} and anything pinned. Older ones keep their text.</span>
      <button type="button" onClick={onChange} className="cursor-pointer font-semibold text-accent-ink hover:underline">Change</button>
      <span className="h-px flex-1 bg-line-soft" />
    </li>
  )
}

const ROW = "group relative grid grid-cols-[84px_minmax(0,1fr)_124px] gap-x-5 py-4 not-first:before:absolute not-first:before:top-0 not-first:before:right-0 not-first:before:left-26 not-first:before:h-px not-first:before:bg-line-soft not-first:before:content-['']"

const NONE_SHOWN: ReadonlySet<number> = new Set()

const jobStatus = (job: JobSnapshot) => {
  if (job.queued) return `Waiting · ${job.model}`
  if (job.paused) return 'Paused · dictation goes first'
  return `${job.model} · ${Math.floor(job.progress * 100)}%`
}

/** A job's progress, status and Cancel, one line tall. */
function JobControls({ job, className }: { job: JobSnapshot; className?: string }) {
  return (
    <span className={cn('flex min-w-0 items-center gap-2.5 text-xs text-muted tabular-nums', className)}>
      <span className="relative h-1 min-w-12 max-w-48 flex-1 overflow-hidden rounded-full bg-line-soft">
        <span className="absolute inset-y-0 left-0 rounded-full bg-accent" style={{ width: `${job.progress * 100}%` }} />
      </span>
      <span className="min-w-0 truncate">{jobStatus(job)}</span>
      <button type="button" onClick={() => void cancelJob(job.transcriptId)} className="inline-flex h-6 shrink-0 cursor-pointer items-center gap-1 rounded-md px-2 font-semibold text-fg-2 hover:bg-surface-hover hover:text-fg">
        <XIcon size={11} weight="bold" />Cancel
      </button>
    </span>
  )
}

/** A row's own job. Its own component, so only it wakes for progress. */
function RowJob({ id }: { id: number }) {
  const job = useJob(id)
  if (!job) return null
  return (
    <>
      <span className="h-3.5 w-px shrink-0 bg-line-soft" />
      <JobControls job={job} className="flex-1" />
    </>
  )
}

/** Jobs whose rows a search, filter or feed error hides, so their Cancel is
 *  never out of reach. Shows nothing while every job's row is in the feed. */
function HiddenJobs({ shown }: { shown: ReadonlySet<number> }) {
  const jobs = useJobs()
  const hidden = jobs.filter(job => !shown.has(job.transcriptId))
  return hidden.length > 0 ? <HiddenJobsChip jobs={hidden} /> : null
}

function HiddenJobsChip({ jobs }: { jobs: JobSnapshot[] }) {
  const anchorRef = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  return (
    <>
      <button
        ref={anchorRef}
        type="button"
        aria-expanded={open}
        onClick={() => setOpen(o => !o)}
        className="flex h-6.5 cursor-pointer items-center gap-1.5 rounded-full bg-panel px-2.5 text-xs font-semibold text-fg-2 ring-1 ring-inset ring-line-soft transition-shadow hover:ring-line aria-expanded:ring-line"
      >
        <ArrowsClockwiseIcon size={12} weight="bold" />
        {plural(jobs.length, 'transcription')} running
      </button>
      <Popover open={open} onOpenChange={setOpen} anchorRef={anchorRef} placement="bottom-start" className="w-[28rem]">
        <ul className="py-1">
          {jobs.map(job => (
            <li key={job.transcriptId} className="flex h-10 items-center gap-3 pr-2 pl-3.5">
              {job.kind === 'import'
                ? <FileAudioIcon size={14} weight="duotone" className="shrink-0 text-fg-2" />
                : <ArrowsClockwiseIcon size={14} className="shrink-0 text-fg-2" />}
              <span className="min-w-0 flex-1 truncate text-xs font-semibold text-fg-2">{job.label}</span>
              <JobControls job={job} className="w-72 shrink-0" />
            </li>
          ))}
        </ul>
      </Popover>
    </>
  )
}

const TranscriptRow = memo(function TranscriptRow({ item, onDelete }: { item: Transcript; onDelete: (id: number) => void }) {
  const { clock, meridiem } = splitClock(parseSqliteDate(item.createdAt))
  // A pending import is busy before its job shows in the list too.
  const busy = useHasJob(item.id) || item.pending
  const parts = [
    item.sourceName
      ? (
        <span key="source" className="inline-flex items-center gap-1 font-semibold text-fg-2">
          <FileAudioIcon size={12} weight="bold" /><span className="max-w-56 truncate">{item.sourceName}</span>
        </span>
      )
      : <span key="source" className="font-semibold text-fg-2">{item.targetApp ?? 'Clipboard'}</span>,
    !item.pending && <span key="words">{plural(item.wordCount, 'word')}</span>,
    !item.audioPath && item.durationSeconds != null && <span key="seconds">{fmtTime(Math.round(item.durationSeconds))}</span>,
    item.pinned && (
      <span key="pinned" className="inline-flex items-center gap-1 font-semibold text-accent-ink">
        <PushPinIcon size={11} weight="fill" />Pinned
      </span>
    ),
  ].filter(Boolean)
  return (
    <li className={ROW}>
      <p className="text-xl leading-tight font-bold tracking-tight text-fg tabular-nums">
        {clock}{meridiem && <small className="ml-0.5 text-xs font-semibold tracking-normal text-muted">{meridiem}</small>}
      </p>
      <div className="min-w-0">
        {item.pending ? (
          // No text yet to take the shape of: three lines, as a guess.
          <div aria-hidden="true" className="flex h-[4.875em] flex-col justify-center gap-2.5 text-[14.5px]">
            <div className="h-3.5 w-full animate-pulse rounded bg-surface" />
            <div className="h-3.5 w-full animate-pulse rounded bg-surface" />
            <div className="h-3.5 w-2/3 animate-pulse rounded bg-surface" />
          </div>
        ) : (
          <p className="text-[14.5px] leading-relaxed text-pretty text-fg-2">
            <span aria-busy={busy} className={cn('text-skeleton', busy && 'animate-pulse')}>{item.content}</span>
          </p>
        )}
        <div className="mt-1.5 flex min-h-6 items-center gap-2.5 text-xs text-muted tabular-nums">
          <span className="flex min-w-0 items-center gap-1.5">
            {parts.map((part, i) => <Fragment key={i}>{i > 0 && <span className="text-line">·</span>}{part}</Fragment>)}
          </span>
          {busy
            ? <RowJob id={item.id} />
            : item.audioPath && (
              <>
                <span className="h-3.5 w-px bg-line-soft" />
                <AudioPlayer id={item.id} path={item.audioPath} seconds={item.durationSeconds} />
              </>
            )}
        </div>
      </div>
      <div className="-mt-1 -mr-2 flex justify-self-end gap-0.5 opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100 has-aria-expanded:opacity-100">
        {!busy && <CopyButton text={item.content} />}
        {!busy && item.audioPath && <RetranscribeButton id={item.id} />}
        {item.audioPath && <PinButton id={item.id} pinned={item.pinned} />}
        <button
          type="button"
          aria-label="Delete"
          onClick={() => onDelete(item.id)}
          className={cn(ACTION_BUTTON, 'hover:bg-danger-soft hover:text-danger')}
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
const DayWave = memo(function DayWave() {
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
})

type EmptyFeedProps = { searching: boolean; dated: boolean; hotkey: string | null; onClearFilters: () => void }

/** The empty feed says why it is empty: a search, a date range, or no dictations yet. */
function EmptyFeed({ searching, dated, hotkey, onClearFilters }: EmptyFeedProps) {
  const narrowed = searching || dated
  const hint = searching
    ? dated ? 'Try different keywords or a wider date range.' : 'Try different keywords or check your spelling.'
    : 'Try a wider date range.'
  return (
    <div className="flex flex-col items-center gap-3 px-6 py-14 text-center">
      <span className="grid size-14 place-items-center rounded-full bg-accent-soft text-accent-ink">
        {narrowed ? <MagnifyingGlassIcon size={24} /> : <MicrophoneIcon size={24} weight="duotone" />}
      </span>
      <p className="text-md font-semibold text-fg">
        {searching ? 'No results found' : dated ? 'Nothing in this date range' : 'Your first dictation lands here'}
      </p>
      <p className="max-w-64 text-sm text-muted text-balance">
        {narrowed ? hint : <>Hold {hotkey ? <HotkeyKeys hotkey={hotkey} /> : 'your hotkey'} in any text box, speak, and let go.</>}
      </p>
      {dated && <Button size="sm" variant="outline" onClick={onClearFilters}>Clear date filter</Button>}
    </div>
  )
}

export function Dashboard() {
  const keys = useSlice('settings', settings => settings.hotkeys.pushToTalk)
  const missingHotkey = keys === null
  const navigate = useNavigate()
  const [searchTerm, setSearchTerm] = useState('')
  const typedRef = useRef('')
  const [filters, setFilters] = useState<TranscriptFilters>(NO_FILTERS)
  const observerRef = useRef<IntersectionObserver | null>(null)

  const isSearchMode = searchTerm.length > 0
  const active = useHistoryFeed(searchTerm, filters)
  const stats = useStats()
  const deleteTranscript = useDeleteTranscript()
  // Both read the same database: when both fail, one error says it once.
  const bothFailed = stats.status === 'error' && stats.data == null && active.status === 'error'
  const retryFeed = bothFailed
    ? () => { void stats.refetch(); void active.refetch() }
    : active.refetch


  // A ref callback, not an effect: the sentinel mounts only after the feed's
  // skeleton is replaced, which changes no effect dependency — an effect would
  // read a null ref and never re-run.
  const { fetchNextPage, hasNextPage, isFetching } = active
  const keep = useSlice('settings', settings => (settings.savedAudio.enabled ? settings.savedAudio.keep : 0))
  // Only an unfiltered feed shows where the audio limit falls.
  const wholeFeed = !isSearchMode && !isFiltered(filters)
  const { groups, boundaryId, seen } = useMemo(() => {
    const rows = active.data?.pages.flatMap(page => page.rows) ?? []
    const seen = new Set<number>()
    const unique = rows.filter(row => !seen.has(row.id) && seen.add(row.id))
    return { groups: groupByDay(unique), boundaryId: wholeFeed ? audioBoundary(unique, keep, hasNextPage) : null, seen }
  }, [active.data, keep, wholeFeed, hasNextPage])
  // A feed error shows its message instead of the rows.
  const shown = active.status === 'success' ? seen : NONE_SHOWN
  const canFetchNext = hasNextPage && !isFetching
  const sentinelRef = useCallback((node: HTMLDivElement | null) => {
    observerRef.current?.disconnect()
    if (!node || !canFetchNext) return
    observerRef.current = new IntersectionObserver(
      // Never cancels a reread in flight: the next page would join the old pages.
      (entries) => { if (entries[0].isIntersecting) void fetchNextPage({ cancelRefetch: false }) },
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
      {missingHotkey && (
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
          {isFiltered(filters) && <FilterChip filters={filters} onClear={() => setFilters(NO_FILTERS)} />}
          <HiddenJobs shown={shown} />
          <div className="ml-auto flex items-center gap-1.5">
            <SearchBox onSearch={setSearchTerm} typedRef={typedRef} />
            <FilterDropdown filters={filters} onChange={setFilters} />
            <ImportControls />
            <ExportButton typedRef={typedRef} filters={filters} />
          </div>
        </div>

        <SectionState
          status={active.status}
          error={active.error?.message}
          onRetry={retryFeed}
          skeleton={<FeedSkeleton />}
        >
          {groups.length === 0 ? (
            <EmptyFeed searching={isSearchMode} dated={hasDates(filters)} hotkey={keys} onClearFilters={() => setFilters(NO_FILTERS)} />
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
                      <Fragment key={item.id}>
                        <TranscriptRow item={item} onDelete={deleteTranscript} />
                        {item.id === boundaryId && (
                          <AudioBoundary keep={keep} onChange={() => navigate(ROUTES.SETTINGS, { state: { tab: 'general' } })} />
                        )}
                      </Fragment>
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
