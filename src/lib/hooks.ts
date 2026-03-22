import { useEffect, useLayoutEffect, useRef } from 'react'
import { listen, type EventName } from '@tauri-apps/api/event'

/** Registers a Tauri event listener and cleans up on unmount. */
export function useEventListener<T>(event: EventName, handler: (payload: T) => void) {
  const handlerRef = useRef(handler)
  useLayoutEffect(() => { handlerRef.current = handler })

  useEffect(() => {
    const unlisten = listen<T>(event, (e) => handlerRef.current(e.payload))
    return () => { unlisten.then(fn => fn()) }
  }, [event])
}

/** Calls callback when a mousedown occurs outside the given ref. */
export function useClickOutside<T extends HTMLElement>(
  ref: React.RefObject<T | null>,
  callback: () => void,
  enabled = true
) {
  const callbackRef = useRef(callback)
  useLayoutEffect(() => { callbackRef.current = callback })

  useEffect(() => {
    if (!enabled) return
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        callbackRef.current()
      }
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [ref, enabled])
}
