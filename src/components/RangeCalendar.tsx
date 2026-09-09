import { useMemo, useState } from 'react'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { isoDay, parseDay } from '../lib/dates'

const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December']
const WEEKDAYS = ['M', 'T', 'W', 'T', 'F', 'S', 'S']

/** Monday-first offset for the 1st of the month. */
function leadingBlanks(year: number, month: number) {
  return (new Date(year, month, 1).getDay() + 6) % 7
}

type Props = {
  from: string | null
  to: string | null
  /** Fires on every click: first sets the start, second completes the span. */
  onChange: (from: string | null, to: string | null) => void
}

/**
 * A two-click range calendar. Click a day to start, click another to close the
 * span; clicking a third time starts over. Selecting backwards is allowed and
 * silently normalised, because insisting on chronological order for a
 * two-click gesture only creates a wrong-way error to recover from.
 *
 * Months apart are reachable by paging the header, which is the case a pair of
 * text fields handled worst.
 */
export function RangeCalendar({ from, to, onChange }: Props) {
  const [cursor, setCursor] = useState(() => {
    const base = parseDay(from) ?? new Date()
    return { year: base.getFullYear(), month: base.getMonth() }
  })
  // Set while a start is chosen but the span is still open, so the grid can
  // preview the range under the pointer.
  const [pendingStart, setPendingStart] = useState<string | null>(null)
  const [hovered, setHovered] = useState<string | null>(null)

  const today = isoDay(new Date())

  const days = useMemo(() => {
    const count = new Date(cursor.year, cursor.month + 1, 0).getDate()
    return Array.from({ length: count }, (_, i) => new Date(cursor.year, cursor.month, i + 1))
  }, [cursor])

  const step = (delta: number) => setCursor(c => {
    const d = new Date(c.year, c.month + delta, 1)
    return { year: d.getFullYear(), month: d.getMonth() }
  })

  const pick = (iso: string) => {
    if (pendingStart === null) {
      setPendingStart(iso)
      onChange(iso, iso)
      return
    }
    const [a, b] = pendingStart <= iso ? [pendingStart, iso] : [iso, pendingStart]
    setPendingStart(null)
    setHovered(null)
    onChange(a, b)
  }

  // While a span is open the preview follows the pointer; otherwise it is the
  // committed selection.
  const previewEnd = pendingStart !== null ? (hovered ?? pendingStart) : null
  const lo = pendingStart !== null && previewEnd
    ? (pendingStart <= previewEnd ? pendingStart : previewEnd)
    : from
  const hi = pendingStart !== null && previewEnd
    ? (pendingStart <= previewEnd ? previewEnd : pendingStart)
    : to

  return (
    <div className="select-none">
      <div className="flex items-center justify-between px-1 pb-2">
        <button
          type="button" onClick={() => step(-1)} aria-label="Previous month"
          className="iconbtn size-6"
        >
          <ChevronLeft size={13} strokeWidth={2.25} />
        </button>
        <span className="text-[11.5px] font-semibold text-(--fg)">
          {MONTHS[cursor.month]} {cursor.year}
        </span>
        <button
          type="button" onClick={() => step(1)} aria-label="Next month"
          className="iconbtn size-6"
        >
          <ChevronRight size={13} strokeWidth={2.25} />
        </button>
      </div>

      <div className="grid grid-cols-7 gap-y-0.5">
        {WEEKDAYS.map((d, i) => (
          <span key={i} className="grid h-6 place-items-center text-[10px] font-medium text-(--faint)">
            {d}
          </span>
        ))}

        {Array.from({ length: leadingBlanks(cursor.year, cursor.month) }, (_, i) => (
          <span key={`blank-${i}`} />
        ))}

        {days.map(d => {
          const iso = isoDay(d)
          const isStart = iso === lo
          const isEnd = iso === hi
          const inRange = !!lo && !!hi && iso > lo && iso < hi
          const isEdge = isStart || isEnd
          const isToday = iso === today

          return (
            <button
              key={iso}
              type="button"
              onClick={() => pick(iso)}
              onPointerEnter={() => pendingStart !== null && setHovered(iso)}
              aria-label={iso}
              aria-pressed={isEdge || inRange}
              className={[
                'relative grid h-7 place-items-center text-[11.5px] tabular-nums transition-colors duration-(--t-fast)',
                // The span reads as one continuous bar: only the ends round off.
                inRange ? 'bg-(--accent-soft) text-(--on-soft)' : '',
                isStart && !isEnd ? 'rounded-l-(--r-sm) bg-(--accent) font-semibold text-(--accent-fg)' : '',
                isEnd && !isStart ? 'rounded-r-(--r-sm) bg-(--accent) font-semibold text-(--accent-fg)' : '',
                isStart && isEnd ? 'rounded-(--r-sm) bg-(--accent) font-semibold text-(--accent-fg)' : '',
                !isEdge && !inRange ? 'rounded-(--r-sm) text-(--fg-2) hover:bg-(--surface-hover) hover:text-(--fg)' : '',
              ].join(' ')}
            >
              {d.getDate()}
              {isToday && !isEdge && (
                <span className="absolute bottom-0.5 size-1 rounded-full bg-(--accent)" />
              )}
            </button>
          )
        })}
      </div>
    </div>
  )
}
