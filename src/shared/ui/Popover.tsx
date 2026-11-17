import {
  useCallback, useEffect, useEffectEvent, useLayoutEffect, useRef, useState,
  type ComponentProps, type CSSProperties, type ReactNode, type RefObject,
} from 'react'
import { createPortal } from 'react-dom'
import { cn } from '@/shared/utils'
import { insideNewerLayer, registerLayer, type Layer } from './dismissLayers'

/** `side` opens beside the anchor (left, or right without room), top-aligned. */
type Placement = 'bottom-start' | 'bottom-end' | 'bottom' | 'side'

type Props = {
  open: boolean
  onOpenChange: (open: boolean) => void
  anchorRef: RefObject<HTMLElement | null>
  /** The trigger, when the panel uses a different element for positioning. */
  focusRef?: RefObject<HTMLElement | null>
  placement?: Placement
  /** As wide as the anchor. */
  matchWidth?: boolean
  /** Gap between anchor and panel. */
  offset?: number
  role?: string
  'aria-label'?: string
  className?: string
  children: ReactNode
}

type Position = { top: number; left: number; width?: number; above: boolean }

const MARGIN = 8

/** A panel anchored to its trigger, in a portal so no scroller clips it. It
 *  flips above the trigger when there is no room below, and closes on Escape
 *  or a click outside it and any popover opened from it. */
export function Popover({
  open, onOpenChange, anchorRef, focusRef = anchorRef, placement = 'bottom-start', matchWidth = false, offset = 6,
  role, 'aria-label': label, className, children,
}: Props) {
  const panelRef = useRef<HTMLDivElement>(null)
  const layerRef = useRef<Layer | null>(null)
  /** Picked on opening and kept, so a resize never flips the panel. */
  const aboveRef = useRef<boolean | null>(null)
  const [pos, setPos] = useState<Position | null>(null)
  const dismiss = useEffectEvent(() => {
    onOpenChange(false)
    focusRef.current?.focus()
  })
  const onPointer = useEffectEvent((e: PointerEvent) => {
    const target = e.target as Node
    const inside = [panelRef, anchorRef].some(r => r.current?.contains(target))
      || (layerRef.current !== null && insideNewerLayer(layerRef.current, target))
    if (!inside) onOpenChange(false)
  })

  const place = useCallback(() => {
    const anchor = anchorRef.current
    const panel = panelRef.current
    if (!anchor || !panel) return
    const a = anchor.getBoundingClientRect()
    const w = matchWidth ? a.width : panel.offsetWidth
    const h = panel.offsetHeight
    if (placement === 'side') {
      const left = a.left - offset - w >= MARGIN ? a.left - offset - w : a.right + offset
      const top = Math.min(Math.max(MARGIN, a.top), window.innerHeight - h - MARGIN)
      setPos({ top, left, above: false })
      return
    }
    const room = window.innerHeight - a.bottom - offset - MARGIN
    const above = aboveRef.current ??= h > room && a.top - offset - MARGIN > room
    const top = Math.max(MARGIN, Math.min(above ? a.top - offset - h : a.bottom + offset, window.innerHeight - h - MARGIN))
    let left = placement === 'bottom-end' ? a.right - w : placement === 'bottom' ? a.left + (a.width - w) / 2 : a.left
    left = Math.min(Math.max(MARGIN, left), window.innerWidth - w - MARGIN)
    setPos({ top, left, width: matchWidth ? a.width : undefined, above })
  }, [anchorRef, matchWidth, offset, placement])

  useLayoutEffect(() => {
    if (!open) return
    aboveRef.current = null
    place()
  }, [open, place])

  useLayoutEffect(() => {
    if (!open) return
    const layer: Layer = { escape: () => dismiss(), contains: node => !!panelRef.current?.contains(node) }
    layerRef.current = layer
    const unregister = registerLayer(layer)
    return () => {
      layerRef.current = null
      unregister()
    }
  }, [open])

  useEffect(() => {
    if (!open) return
    const pointer = (event: PointerEvent) => onPointer(event)
    const observer = new ResizeObserver(place)
    if (panelRef.current) observer.observe(panelRef.current)
    document.addEventListener('pointerdown', pointer, true)
    window.addEventListener('resize', place)
    window.addEventListener('scroll', place, true)
    return () => {
      observer.disconnect()
      document.removeEventListener('pointerdown', pointer, true)
      window.removeEventListener('resize', place)
      window.removeEventListener('scroll', place, true)
    }
  }, [open, place])

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
