import { useCallback, useEffect, useLayoutEffect, useRef, useState, type RefObject } from 'react'
import { commands } from '../shared/bindings'

const CAPSULE_H = 32
export const CARD_W = 332
const CARD_MIN_H = 84
const CARD_MAX_H = 186
const CARD_RADIUS = 28
const TEXT_AND_STRIP = 69

const painted = () => new Promise<void>(resolve => {
  requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
})

export function usePillGeometry(wantsCard: boolean, wantsHeadroom: boolean, capsuleWidth: number,
  pillRef: RefObject<HTMLDivElement | null>, innerRef: RefObject<HTMLSpanElement | null>,
  textRef: RefObject<HTMLDivElement | null>) {
  const [roomy, setRoomy] = useState(false)
  // The capsule window has no room above the pill, so a message grows it too.
  const [headroom, setHeadroom] = useState(false)
  const wantsHeadroomRef = useRef(wantsHeadroom)
  const [contentHeight, setContentHeight] = useState(CARD_MIN_H)
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
      await commands.resizePill(large)
      windowExpanded.current = large
    })
    queue.current = next.catch(() => {})
    return next
  }, [])

  useEffect(() => {
    wantsHeadroomRef.current = wantsHeadroom
    if (wantsHeadroom) {
      let cancelled = false
      void resize(true).then(() => {
        if (!cancelled) setHeadroom(true)
      }).catch(error => console.error('Unable to make room for pill message', error))
      return () => { cancelled = true }
    }
    // A card still open or closing owns the shrink.
    if (!headroom || wantsCard || roomy) return
    void resize(false).then(() => setHeadroom(false))
      .catch(error => console.error('Unable to release pill message room', error))
  }, [wantsHeadroom, headroom, wantsCard, roomy, resize])

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
        setContentHeight(next)
      }
      const text = textRef.current
      if (text) text.scrollTop = text.scrollHeight
    }
    measure()
    const observer = new ResizeObserver(entries => {
      if (entries.some(entry => entry.target === inner)) measure()
      // Follow the newest line as CSS changes the transcript viewport's
      // height, without a JS animation loop or React render per frame.
      const text = textRef.current
      if (text) text.scrollTop = text.scrollHeight
    })
    observer.observe(inner)
    if (textRef.current) observer.observe(textRef.current)
    return () => observer.disconnect()
  }, [expanded, innerRef, textRef])

  useLayoutEffect(() => {
    if (wantsCard) return
    let cancelled = false
    // getAnimations flushes the new CSS targets. Wait for the shell's actual
    // transitions before shrinking its native window, including a reversed
    // transition. Reduced motion produces no transitions and closes at once.
    const transitions = pillRef.current?.getAnimations().filter(animation =>
      'transitionProperty' in animation && /^(width|height|border-.*-radius)$/.test(animation.transitionProperty as string),
    ) ?? []
    void Promise.allSettled(transitions.map(animation => animation.finished)).then(async () => {
      if (cancelled) return
      // Also runs if opening ended before roomy became true. A reopen cancels
      // this completion; the serialized resize queue handles IPC in flight.
      setRoomy(false)
      if (!wantsHeadroomRef.current) await resize(false)
    }).catch(error => console.error('Unable to collapse recording pill', error))
    return () => { cancelled = true }
  }, [wantsCard, capsuleWidth, pillRef, resize])

  useEffect(() => () => { void resize(false).catch(error => console.error('Unable to release pill window', error)) }, [resize])

  return {
    roomy, expanded, headroom,
    width: expanded ? CARD_W : capsuleWidth,
    height: expanded ? contentHeight : CAPSULE_H,
    radius: expanded ? CARD_RADIUS : CAPSULE_H / 2,
  }
}
