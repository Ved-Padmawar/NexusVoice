import { useEffect, useLayoutEffect, useRef, useState } from 'react'
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

/** Registers a Tauri event listener and cleans up on unmount. */
export function useEventListener<T>(event: EventName, handler: (payload: T) => void) {
  const handlerRef = useRef(handler)
  useLayoutEffect(() => { handlerRef.current = handler })

  useEffect(() => {
    const unlisten = listen<T>(event, (e) => handlerRef.current(e.payload))
    return () => { unlisten.then(fn => fn()).catch(() => {}) }
  }, [event])
}
