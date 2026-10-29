import { useLayoutEffect, useRef, type ReactNode } from 'react'
import { cn } from '@/shared/utils'

export type SegmentedOption<T extends string> = { value: T; label: ReactNode }

type Props<T extends string> = {
  value: T
  options: readonly SegmentedOption<T>[]
  onChange: (value: T) => void
  /** `md` for page-level tabs, `sm` for filters and in-menu toggles. */
  size?: 'md' | 'sm'
  /** Equal-width columns across the full width, instead of hugging the labels. */
  fill?: boolean
  /** `tabs` for the Settings tabs; `radio` for everything else. */
  kind?: 'tabs' | 'radio'
  'aria-label'?: string
  className?: string
}

/** One thumb that slides to the selected option. It is placed relative to the
 *  track, so it moves only when the selection does, never with the track (a
 *  popover sliding into place). Ease out, never overshoot. */
export function Segmented<T extends string>({
  value, options, onChange, size = 'md', fill = false, kind = 'radio', 'aria-label': label, className,
}: Props<T>) {
  const trackRef = useRef<HTMLDivElement>(null)
  const thumbRef = useRef<HTMLSpanElement>(null)
  const tabs = kind === 'tabs'

  useLayoutEffect(() => {
    const track = trackRef.current
    const thumb = thumbRef.current
    if (!track || !thumb) return
    const place = () => {
      const on = track.querySelector<HTMLElement>('[data-on]')
      // Hidden (a closed popover): nothing to measure, and the next visible
      // placement must jump, not grow from zero.
      if (!on?.offsetWidth) {
        delete thumb.dataset.ready
        return
      }
      thumb.style.translate = `${on.offsetLeft}px ${on.offsetTop}px`
      thumb.style.width = `${on.offsetWidth}px`
      thumb.style.height = `${on.offsetHeight}px`
      // The first placement jumps; only later changes slide.
      requestAnimationFrame(() => { thumb.dataset.ready = '' })
    }
    place()
    // Labels can change width (counts), which moves the selected option.
    const observer = new ResizeObserver(place)
    observer.observe(track)
    return () => observer.disconnect()
  }, [value])

  return (
    <div
      ref={trackRef}
      role={tabs ? 'tablist' : 'radiogroup'}
      aria-label={label}
      className={cn(
        'relative gap-0.5 rounded-[10px] ring-1 ring-inset ring-line-soft',
        fill ? 'grid auto-cols-fr grid-flow-col' : 'inline-flex',
        size === 'md' ? 'bg-panel p-1' : 'bg-field p-0.75',
        className,
      )}
    >
      <span
        ref={thumbRef}
        aria-hidden="true"
        className="absolute top-0 left-0 rounded-md bg-accent-soft ring-1 ring-accent-line ring-inset data-ready:transition-[translate,width] data-ready:duration-300 data-ready:ease-out-quint"
      />
      {options.map((o) => {
        const on = o.value === value
        return (
          <button
            key={o.value}
            type="button"
            role={tabs ? 'tab' : 'radio'}
            aria-selected={tabs ? on : undefined}
            aria-checked={tabs ? undefined : on}
            data-on={on || undefined}
            onClick={() => onChange(o.value)}
            className={cn(
              'relative inline-flex cursor-pointer items-center justify-center gap-1.5 rounded-md font-semibold whitespace-nowrap transition-colors select-none',
              size === 'md' ? 'h-7.5 px-3.5 text-sm' : 'h-6.5 px-3 text-xs',
              on ? 'text-accent-ink' : 'text-muted hover:text-fg',
            )}
          >
            {o.label}
          </button>
        )
      })}
    </div>
  )
}
