import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from 'react'
import { listen, type EventName } from '@tauri-apps/api/event'

/**
 * Returns `true` only after `active` has stayed truthy for `delayMs`.
 * Used to gate loaders so sub-second waits never flash a spinner/skeleton.
 * Flips back to `false` immediately when `active` becomes false.
 */
export function useDelayedFlag(active: boolean, delayMs = 250): boolean {
  const [shown, setShown] = useState(false)
  useEffect(() => {
    if (!active) return
    const t = setTimeout(() => setShown(true), delayMs)
    return () => {
      clearTimeout(t)
      setShown(false)
    }
  }, [active, delayMs])
  return shown
}

/** The page scroller every route scrolls inside. */
export const SCROLLER_SELECTOR = '.nv-main'

/**
 * Whether a sticky element is pinned. Measured on scroll, not with an
 * IntersectionObserver, so it can't collide with the feed's sentinel observer.
 */
export function useStuck(ref: RefObject<HTMLElement | null>): boolean {
  const [stuck, setStuck] = useState(false)

  useEffect(() => {
    const el = ref.current
    const scroller = el?.closest<HTMLElement>(SCROLLER_SELECTOR)
    if (!el || !scroller) return
    let frame = 0
    const measure = () => {
      frame = 0
      const offset = parseFloat(getComputedStyle(el).top) || 0
      const gap = el.getBoundingClientRect().top - scroller.getBoundingClientRect().top
      setStuck(scroller.scrollTop > 0 && gap <= offset + 0.5)
    }
    const onScroll = () => { if (!frame) frame = requestAnimationFrame(measure) }
    measure()
    scroller.addEventListener('scroll', onScroll, { passive: true })
    return () => {
      scroller.removeEventListener('scroll', onScroll)
      cancelAnimationFrame(frame)
    }
  }, [ref])

  return stuck
}

/** Registers a Tauri event listener and cleans up on unmount. */
export function useEventListener<T>(event: EventName, handler: (payload: T) => void) {
  const handlerRef = useRef(handler)
  useLayoutEffect(() => { handlerRef.current = handler })

  useEffect(() => {
    const unlisten = listen<T>(event, (e) => handlerRef.current(e.payload))
    return () => { unlisten.then(fn => fn()).catch(() => {}) }
  }, [event])
}
