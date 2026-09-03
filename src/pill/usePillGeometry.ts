import { useCallback, useEffect, useLayoutEffect, useRef, useState, type RefObject } from 'react'
import { animate, useMotionValue } from 'framer-motion'
import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from '../lib/commands'

const CAPSULE_H = 32
export const CARD_W = 332
const CARD_MIN_H = 84
const CARD_MAX_H = 186
const CARD_RADIUS = 28
const TEXT_AND_STRIP = 69

// Monotone curves cannot overshoot, including a reversal mid-transition.
// Geometry has one owner and one clock; CSS only eases colour and opacity.
const SHAPE_TRANSITION = { type: 'tween' as const, duration: 0.28, ease: [0.22, 1, 0.36, 1] as [number, number, number, number] }

const painted = () => new Promise<void>(resolve => {
  requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
})

export function usePillGeometry(wantsCard: boolean, capsuleWidth: number, reduceMotion: boolean | null,
  innerRef: RefObject<HTMLSpanElement | null>, textRef: RefObject<HTMLDivElement | null>) {
  const [roomy, setRoomy] = useState(false)
  const [contentHeight, setContentHeight] = useState(CARD_MIN_H)
  const heightRef = useRef(CARD_MIN_H)
  const width = useMotionValue(capsuleWidth)
  const height = useMotionValue(CAPSULE_H)
  const radius = useMotionValue(CAPSULE_H / 2)
  const queue = useRef<Promise<void>>(Promise.resolve())
  // A renderer reload can inherit an expanded OS window. Establish its size
  // rather than assuming that a fresh React tree means a fresh window.
  const windowExpanded = useRef<boolean | null>(null)
  const expanded = wantsCard && roomy

  // Preserve IPC order across rapid close/reopen. A failed resize remains
  // retryable; never mark the window small before the OS confirms it.
  const resize = useCallback((large: boolean) => {
    const next = queue.current.then(async () => {
      if (windowExpanded.current === large) return
      await invoke(COMMANDS.RESIZE_PILL, { expanded: large })
      windowExpanded.current = large
    })
    queue.current = next.catch(() => {})
    return next
  }, [])

  useEffect(() => {
    if (!wantsCard) return
    let cancelled = false
    void resize(true).then(painted).then(() => {
      if (!cancelled) setRoomy(true)
    }).catch(error => console.error('Unable to expand recording pill', error))
    return () => { cancelled = true }
  }, [wantsCard, resize])

  useLayoutEffect(() => {
    if (!expanded) return
    const inner = innerRef.current
    if (!inner) return
    const measure = () => {
      const measured = inner.offsetHeight
      if (measured > 0) {
        const next = Math.min(CARD_MAX_H, Math.max(CARD_MIN_H, measured + TEXT_AND_STRIP))
        heightRef.current = next
        setContentHeight(next)
      }
      const text = textRef.current
      if (text) text.scrollTop = text.scrollHeight
    }
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(inner)
    return () => observer.disconnect()
  }, [expanded, innerRef, textRef])

  useLayoutEffect(() => {
    let cancelled = false
    const transition = reduceMotion ? { duration: 0 } : SHAPE_TRANSITION
    const controls = [
      animate(width, expanded ? CARD_W : capsuleWidth, transition),
      animate(height, expanded ? heightRef.current : CAPSULE_H, transition),
      animate(radius, expanded ? CARD_RADIUS : CAPSULE_H / 2, transition),
    ]
    if (!wantsCard) {
      void Promise.all(controls).then(async () => {
        if (cancelled) return
        // Also runs when opening was interrupted before roomy became true.
        // Gate a reopen before dispatching the shrink: once it is in flight,
        // the next expansion must wait for its own resize acknowledgement.
        setRoomy(false)
        await resize(false)
      }).catch(error => console.error('Unable to collapse recording pill', error))
    }
    return () => { cancelled = true; controls.forEach(control => control.stop()) }
  }, [expanded, wantsCard, capsuleWidth, reduceMotion, width, height, radius, resize])

  useLayoutEffect(() => {
    if (!expanded) return
    const control = animate(height, contentHeight, reduceMotion ? { duration: 0 } : SHAPE_TRANSITION)
    return () => control.stop()
  }, [expanded, contentHeight, reduceMotion, height])

  // Keep the latest line visible as the viewport grows, not just at the start
  // of the resize. No React render is needed for each animation frame.
  useEffect(() => height.on('change', () => {
    const text = textRef.current
    if (text && wantsCard) text.scrollTop = text.scrollHeight
  }), [height, textRef, wantsCard])

  useEffect(() => () => { void resize(false).catch(error => console.error('Unable to release pill window', error)) }, [resize])

  return { roomy, expanded, width, height, radius }
}
