import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import type { EventCallback, UnlistenFn } from '@tauri-apps/api/event'
import { events } from './bindings'

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

type BackendEvent<T> = {
  listen: (handler: EventCallback<T>) => Promise<UnlistenFn>
}

type BackendEventName = keyof typeof events
type BackendPayload<K extends BackendEventName> = Parameters<Parameters<typeof events[K]['listen']>[0]>[0]['payload']

/**
 * Listen by generated key: tauri-specta's event proxy creates a new object on
 * every access. Depending on that object would resubscribe on every render,
 * losing one-shot events while the replacement listener's IPC is in flight.
 */
export function useEventListener<K extends BackendEventName>(event: K, handler: (payload: BackendPayload<K>) => void) {
  const handlerRef = useRef(handler)
  useLayoutEffect(() => { handlerRef.current = handler })

  useEffect(() => {
    let cancelled = false
    const source = events[event] as BackendEvent<BackendPayload<K>>
    const unlisten = source.listen((e) => {
      if (!cancelled) handlerRef.current(e.payload)
    })
    // Attach a rejection handler immediately, even when the component remains mounted.
    void unlisten.catch(() => {})
    return () => {
      cancelled = true
      void unlisten.then(fn => fn()).catch(() => {})
    }
  }, [event])
}
