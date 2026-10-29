import {
  useCallback, useEffect, useId, useLayoutEffect, useRef, useState,
  type ComponentProps, type CSSProperties, type ReactNode, type RefObject,
} from 'react'
import { createPortal } from 'react-dom'
import { cn } from '@/shared/utils'

type Placement = 'bottom-start' | 'bottom-end' | 'bottom'

type Props = {
  open: boolean
  onOpenChange: (open: boolean) => void
  anchorRef: RefObject<HTMLElement | null>
  placement?: Placement
  /** As wide as the anchor. */
  matchWidth?: boolean
  /** Gap between anchor and panel. */
  offset?: number
  /** Clicks here don't count as outside (another popover opened from this one). */
  insideRefs?: RefObject<HTMLElement | null>[]
  role?: string
  'aria-label'?: string
  className?: string
  children: ReactNode
}

type Position = { top: number; left: number; width?: number; above: boolean }

const MARGIN = 8

/** Open popovers, innermost last. */
const openStack: string[] = []

/** A panel anchored to its trigger, in a portal so no scroller clips it. It
 *  flips above the trigger when there is no room below, and closes on Escape
 *  or a click elsewhere. */
export function Popover({
  open, onOpenChange, anchorRef, placement = 'bottom-start', matchWidth = false, offset = 6,
  insideRefs, role, 'aria-label': label, className, children,
}: Props) {
  const panelRef = useRef<HTMLDivElement>(null)
  const [pos, setPos] = useState<Position | null>(null)

  const place = useCallback(() => {
    const anchor = anchorRef.current
    const panel = panelRef.current
    if (!anchor || !panel) return
    const a = anchor.getBoundingClientRect()
    const w = matchWidth ? a.width : panel.offsetWidth
    const h = panel.offsetHeight
    const room = window.innerHeight - a.bottom - offset - MARGIN
    const above = h > room && a.top - offset - MARGIN > room
    const top = above ? a.top - offset - h : a.bottom + offset
    let left = placement === 'bottom-end' ? a.right - w : placement === 'bottom' ? a.left + (a.width - w) / 2 : a.left
    left = Math.min(Math.max(MARGIN, left), window.innerWidth - w - MARGIN)
    setPos({ top, left, width: matchWidth ? a.width : undefined, above })
  }, [anchorRef, matchWidth, offset, placement])

  useLayoutEffect(() => {
    if (open) place()
  }, [open, place])

  const id = useId()
  useEffect(() => {
    if (!open) return
    openStack.push(id)
    const onPointer = (e: PointerEvent) => {
      const t = e.target as Node
      const inside = [panelRef, anchorRef, ...(insideRefs ?? [])].some((r) => r.current?.contains(t))
      if (!inside) onOpenChange(false)
    }
    // Escape closes only the innermost popover (a calendar, not the filter under it).
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || openStack.at(-1) !== id) return
      e.stopPropagation()
      onOpenChange(false)
      anchorRef.current?.focus()
    }
    const observer = new ResizeObserver(place)
    if (panelRef.current) observer.observe(panelRef.current)
    document.addEventListener('pointerdown', onPointer, true)
    document.addEventListener('keydown', onKey)
    window.addEventListener('resize', place)
    window.addEventListener('scroll', place, true)
    return () => {
      openStack.splice(openStack.indexOf(id), 1)
      observer.disconnect()
      document.removeEventListener('pointerdown', onPointer, true)
      document.removeEventListener('keydown', onKey)
      window.removeEventListener('resize', place)
      window.removeEventListener('scroll', place, true)
    }
  }, [open, id, place, onOpenChange, anchorRef, insideRefs])

  // Always mounted: CSS fades it in and out, holding `display` until the fade ends.
  return createPortal(
    <div
      ref={panelRef}
      role={role}
      aria-label={label}
      data-open={open || undefined}
      aria-hidden={!open}
      inert={!open}
      className={cn(
        'fixed z-50 hidden translate-y-(--dy) overflow-hidden rounded-xl bg-panel text-fg opacity-0 shadow-(--shadow-md) outline outline-line -outline-offset-1',
        'transition-[opacity,translate,display] transition-discrete duration-200 ease-out-quint',
        'data-open:block data-open:translate-y-0 data-open:opacity-100',
        'starting:data-open:translate-y-(--dy) starting:data-open:opacity-0',
        className,
      )}
      // Hidden until first placed (before paint), so it never shows in the wrong place.
      style={{
        ...(pos ? { top: pos.top, left: pos.left, width: pos.width } : { top: 0, left: 0, visibility: 'hidden' }),
        ['--dy' as string]: pos?.above ? '4px' : '-4px',
      } satisfies CSSProperties}
    >
      {children}
    </div>,
    document.body,
  )
}

/** A full-width row in a menu: options touch the menu's edges. */
export function MenuItem({ className, type = 'button', ...props }: ComponentProps<'button'>) {
  return (
    <button
      type={type}
      className={cn(
        'flex h-9 w-full cursor-pointer items-center gap-2.5 px-3.5 text-left text-sm text-fg-2 outline-none hover:bg-surface-hover hover:text-fg focus-visible:bg-surface-hover focus-visible:text-fg disabled:pointer-events-none disabled:opacity-50',
        className,
      )}
      {...props}
    />
  )
}
