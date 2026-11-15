import { useMemo, useRef, useState, type MouseEvent } from 'react'
import { ArrowCounterClockwiseIcon, CalendarBlankIcon, CaretLeftIcon, CaretRightIcon, FunnelSimpleIcon, XIcon } from '@phosphor-icons/react'
import { Button } from '@/shared/ui/Button'
import { Popover } from '@/shared/ui/Popover'
import { Segmented } from '@/shared/ui/Segmented'
import { cn, localDateKey as toIso } from '../../shared/utils'
import { isFiltered, type TranscriptFilters } from '../../shared/queryKeys'
import { NO_FILTERS } from './queries'

export function ToolbarButton({ className, ...props }: React.ComponentProps<'button'>) {
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

type DateMode = 'range' | 'on'
type DateKey = 'from' | 'to' | 'on'
type Dates = Record<DateKey, string>

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
              // Nothing can be dictated tomorrow.
              disabled={v > today}
              onClick={() => onPick(v)}
              className={cn(
                'grid h-7.5 cursor-pointer place-items-center rounded-[7px] text-xs text-fg transition-colors hover:bg-surface-hover disabled:cursor-default disabled:opacity-35 disabled:hover:bg-transparent',
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

function DateFieldButton({ value, expanded, onClick }: {
  value: string
  expanded: boolean
  onClick: (event: MouseEvent<HTMLButtonElement>) => void
}) {
  return (
    <button
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

/** The editor's state for applied filters: a range whose ends match is one day. */
function datesOf({ from, to }: TranscriptFilters): { mode: DateMode; dates: Dates } {
  if (from && from === to) return { mode: 'on', dates: { from: '', to: '', on: from } }
  return { mode: 'range', dates: { from: from ?? '', to: to ?? '', on: '' } }
}

type FilterDropdownProps = {
  filters: TranscriptFilters
  onChange: (filters: TranscriptFilters) => void
}

export function FilterDropdown({ filters, onChange }: FilterDropdownProps) {
  const { sortAsc: filterSortAsc } = filters
  const [open, setOpen] = useState(false)
  const [dateMode, setDateMode] = useState(() => datesOf(filters).mode)
  const [dates, setDates] = useState(() => datesOf(filters).dates)
  const [sortAsc, setSortAsc] = useState(filterSortAsc)
  const [calField, setCalField] = useState<DateKey | null>(null)
  const [calView, setCalView] = useState(() => new Date())
  const hasActive = isFiltered(filters)

  const anchorRef = useRef<HTMLButtonElement>(null)
  const bodyRef = useRef<HTMLDivElement>(null)
  const calRef = useRef<HTMLDivElement>(null)
  const calTrigger = useRef<HTMLButtonElement>(null)
  const calInside = useMemo(() => [calRef], [])

  const handleOpenChange = (next: boolean) => {
    if (next) {
      const applied = datesOf(filters)
      setDateMode(applied.mode)
      setDates(applied.dates)
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
    // Keep the range ordered: picking one end past the other swaps them, so neither is lost.
    if (calField !== 'on' && next.from && next.to && next.from > next.to) {
      [next.from, next.to] = [next.to, next.from]
    }
    setDates(next)
    setCalField(null)
  }

  const apply = () => {
    // Only the visible mode counts, so a cleared day can't fall back to a hidden range.
    if (dateMode === 'on') {
      onChange({ from: dates.on || null, to: dates.on || null, sortAsc })
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
    <DateFieldButton value={dates[key]} expanded={calField === key} onClick={event => {
      calTrigger.current = event.currentTarget
      toggleField(key)
    }} />
  )

  return (
    <>
      <ToolbarButton
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
      </ToolbarButton>
      <Popover open={open} onOpenChange={handleOpenChange} anchorRef={anchorRef} placement="bottom-end" insideRefs={calInside} className="w-72 rounded-2xl p-4">
        <div ref={bodyRef} className="flex flex-col gap-3.5">
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
        // Beside the filter panel, never on top of the fields it edits.
        anchorRef={bodyRef}
        focusRef={calTrigger}
        placement="side"
        offset={22}
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

const chipDateFmt = new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric' })

/** "3 Nov – 9 Nov · Oldest first": what the feed is narrowed to, in words. */
function describeFilters({ from, to, sortAsc }: TranscriptFilters) {
  const day = (iso: string) => chipDateFmt.format(fromIso(iso))
  const dates = from && to
    ? from === to ? `On ${day(from)}` : `${day(from)} – ${day(to)}`
    : from ? `Since ${day(from)}` : to ? `Until ${day(to)}` : null
  return [dates, sortAsc && 'Oldest first'].filter(Boolean).join(' · ')
}

/** Shows that the feed is filtered, and clears it in one click. */
export function FilterChip({ filters, onClear }: { filters: TranscriptFilters; onClear: () => void }) {
  return (
    <button
      type="button"
      onClick={onClear}
      aria-label={`Clear filters: ${describeFilters(filters)}`}
      className="flex h-6.5 cursor-pointer items-center gap-1.5 rounded-full bg-accent-soft pr-2 pl-2.5 text-xs font-semibold text-accent-ink ring-1 ring-inset ring-accent-line transition-shadow hover:ring-accent"
    >
      {describeFilters(filters)}
      <XIcon size={11} weight="bold" />
    </button>
  )
}
