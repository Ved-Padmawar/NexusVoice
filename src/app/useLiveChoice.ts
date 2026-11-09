import { useCallback, useRef, useState } from 'react'
import { readFresh, type Live } from './queryClient'

/**
 * Save a user's pick of a live backend value. The newest pick shows until no
 * save is in flight, then Rust's value is read back. Only the newest pick
 * reports a failure, so a late older reply never undoes a newer pick.
 */
export function useLiveChoice<T, D>(
  options: Live<D>,
  save: (value: T) => Promise<unknown>,
  onError: (error: unknown) => void,
) {
  const [pending, setPending] = useState<{ value: T } | null>(null)
  const saves = useRef({ newest: 0, inFlight: 0 })

  const choose = useCallback(async (value: T) => {
    const current = saves.current
    const pick = ++current.newest
    current.inFlight += 1
    setPending({ value })
    try {
      await save(value)
    } catch (e) {
      if (pick === current.newest) onError(e)
    } finally {
      current.inFlight -= 1
    }
    if (current.inFlight > 0) return
    await readFresh(options).catch(() => {})
    if (current.inFlight === 0) setPending(null)
  }, [options, save, onError])

  return { pending, choose }
}
