import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react'
import { CheckIcon, CaretDownIcon, MagnifyingGlassIcon } from '@phosphor-icons/react'
import { cn } from '@/lib/utils'
import { Popover } from './Popover'

export type SelectOption<T extends string> = {
  value: T
  label: string
  /** A rule under this option (e.g. after "Default" or "Auto-detect"). */
  dividerAfter?: boolean
}

type Props<T extends string> = {
  value: T | null
  options: readonly SelectOption<T>[]
  onChange: (value: T) => void
  'aria-label': string
  /** Leading icon inside the trigger. */
  icon?: ReactNode
  placeholder?: string
  /** `md`: a full field. `sm`: nested inside a chip. */
  size?: 'md' | 'sm'
  disabled?: boolean
  /** A search field appears when the list is longer than this. */
  searchAfter?: number
  searchPlaceholder?: string
  className?: string
}

/** A listbox in a popover. Arrow keys move, Enter picks, typing searches when
 *  the list is long enough to need it. */
export function Select<T extends string>({
  value, options, onChange, 'aria-label': label, icon, placeholder = 'Select…', size = 'md', disabled,
  searchAfter = 12, searchPlaceholder = 'Search', className,
}: Props<T>) {
  const listId = useId()
  const triggerRef = useRef<HTMLButtonElement>(null)
  const searchRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [active, setActive] = useState(0)

  const searchable = options.length > searchAfter
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase()
    return q ? options.filter((o) => o.label.toLowerCase().includes(q) || o.value.toLowerCase().includes(q)) : options
  }, [options, query])
  const current = options.find((o) => o.value === value)

  const openList = () => {
    setQuery('')
    setActive(Math.max(0, options.findIndex((o) => o.value === value)))
    setOpen(true)
  }

  useEffect(() => {
    if (!open) return
    const frame = requestAnimationFrame(() => {
      if (searchable) searchRef.current?.focus()
      else listRef.current?.focus()
      listRef.current?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' })
    })
    return () => cancelAnimationFrame(frame)
  }, [open, searchable])

  useEffect(() => {
    listRef.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: 'nearest' })
  }, [active])

  const pick = (v: T) => {
    onChange(v)
    setOpen(false)
    triggerRef.current?.focus()
  }

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'ArrowDown') { e.preventDefault(); setActive((i) => Math.min(shown.length - 1, i + 1)) }
    else if (e.key === 'ArrowUp') { e.preventDefault(); setActive((i) => Math.max(0, i - 1)) }
    else if (e.key === 'Enter') { e.preventDefault(); const o = shown[active]; if (o) pick(o.value) }
  }

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        disabled={disabled}
        onClick={() => (open ? setOpen(false) : openList())}
        onKeyDown={(e) => { if (!open && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) { e.preventDefault(); openList() } }}
        className={cn(
          'relative flex w-full min-w-0 cursor-pointer items-center gap-2 text-left text-fg ring-1 ring-line-soft ring-inset transition-shadow hover:ring-line disabled:cursor-not-allowed disabled:opacity-50',
          size === 'md' ? 'h-9 rounded-lg bg-field pr-3 pl-3 text-sm' : 'h-6 rounded-md bg-field pr-2 pl-2.5 text-xs font-medium',
          icon && size === 'md' && 'pl-9',
          open && 'ring-accent hover:ring-accent',
          className,
        )}
      >
        {icon && <span className="pointer-events-none absolute left-3 flex text-sm text-accent-ink">{icon}</span>}
        <span className={cn('min-w-0 flex-1 truncate', !current && 'text-muted')}>{current?.label ?? placeholder}</span>
        <CaretDownIcon
          size={size === 'md' ? 12 : 11}
          weight="bold"
          className={cn('shrink-0 text-muted transition-transform duration-200 ease-out-quint', open && 'rotate-180')}
        />
      </button>

      <Popover open={open} onOpenChange={setOpen} anchorRef={triggerRef} matchWidth={size === 'md'} offset={4} className={cn(size === 'sm' && 'min-w-32')}>
        {searchable && (
          <div className="relative border-b border-line-soft p-2">
            <MagnifyingGlassIcon size={14} className="pointer-events-none absolute top-1/2 left-4.5 -translate-y-1/2 text-muted" />
            <input
              ref={searchRef}
              type="search"
              value={query}
              onChange={(e) => { setQuery(e.target.value); setActive(0) }}
              onKeyDown={onKeyDown}
              placeholder={searchPlaceholder}
              aria-label={searchPlaceholder}
              aria-controls={listId}
              className="h-8 w-full rounded-lg bg-field pr-2.5 pl-8 text-sm text-fg ring-1 ring-line-soft ring-inset outline-none placeholder:text-muted focus:ring-accent"
            />
          </div>
        )}
        <div
          ref={listRef}
          id={listId}
          role="listbox"
          aria-label={label}
          tabIndex={-1}
          onKeyDown={onKeyDown}
          className="max-h-70 overflow-y-auto overscroll-none outline-none"
        >
          {shown.map((o, i) => {
            const selected = o.value === value
            return (
              <div key={o.value}>
                <div
                  role="option"
                  aria-selected={selected}
                  data-index={i}
                  onClick={() => pick(o.value)}
                  onPointerMove={() => setActive(i)}
                  className={cn(
                    'flex min-h-8.5 cursor-pointer items-center gap-2 px-3 text-sm',
                    i === active ? 'bg-surface-hover text-fg' : 'text-fg-2',
                    selected && 'font-semibold text-accent-ink',
                  )}
                >
                  <span className="min-w-0 flex-1 truncate">{o.label}</span>
                  {selected && <CheckIcon size={13} className="shrink-0" weight="bold" />}
                </div>
                {o.dividerAfter && !query && <hr className="border-line-soft" />}
              </div>
            )
          })}
          {shown.length === 0 && <p className="px-3 py-3 text-sm text-muted">No matches.</p>}
        </div>
      </Popover>
    </>
  )
}
