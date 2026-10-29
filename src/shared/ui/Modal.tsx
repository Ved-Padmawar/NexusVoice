import { useEffect, useRef, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { cn } from '@/shared/utils'

type Props = {
  open: boolean
  /** Omit for a modal that must be answered (no Escape, no scrim click). */
  onClose?: () => void
  'aria-labelledby'?: string
  /** Panel classes: width, max height, layout. */
  className?: string
  children: ReactNode
}

/** A dialog over the window: a blurred scrim and a panel that scales in. Always
 *  mounted: CSS animates both ways, holding `display` until the exit ends. */
export function Modal({ open, onClose, 'aria-labelledby': labelledBy, className, children }: Props) {
  const panelRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const previous = document.activeElement as HTMLElement | null
    panelRef.current?.focus()
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && onClose) onClose()
      if (e.key !== 'Tab' || !panelRef.current) return
      // Keep Tab inside the dialog: wrap at either end.
      const items = panelRef.current.querySelectorAll<HTMLElement>(
        'a[href], button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])',
      )
      const first = items[0]
      const last = items[items.length - 1]
      const active = document.activeElement
      if (!first || !last) {
        e.preventDefault()
      } else if (e.shiftKey && (active === first || active === panelRef.current)) {
        e.preventDefault()
        last.focus()
      } else if (!e.shiftKey && active === last) {
        e.preventDefault()
        first.focus()
      }
    }
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('keydown', onKey)
      previous?.focus()
    }
  }, [open, onClose])

  return createPortal(
    <div
      data-open={open || undefined}
      inert={!open}
      className="group fixed inset-0 z-50 hidden place-items-center transition-[display] transition-discrete duration-260 data-open:grid"
    >
      <div
        aria-hidden="true"
        onClick={onClose}
        className="absolute inset-0 bg-[rgb(var(--shadow-rgb)/0.55)] opacity-0 backdrop-blur-[3px] transition-opacity duration-160 group-data-open:opacity-100 starting:group-data-open:opacity-0"
      />
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-hidden={!open}
        aria-labelledby={labelledBy}
        tabIndex={-1}
        className={cn(
          'relative flex max-h-[calc(100%-64px)] scale-97 flex-col overflow-hidden rounded-2xl bg-panel text-fg opacity-0 shadow-(--shadow-lg) ring-1 ring-line outline-none',
          'transition-[opacity,scale] duration-[160ms,260ms] ease-out-quint',
          'group-data-open:scale-100 group-data-open:opacity-100 starting:group-data-open:scale-97 starting:group-data-open:opacity-0',
          className,
        )}
      >
        {children}
      </div>
    </div>,
    document.body,
  )
}
