import { useCallback, useSyncExternalStore } from 'react'
import { hashKey, type QueryKey } from '@tanstack/react-query'
import { readAfresh } from './sync'

type Pick = { value: unknown; save: (value: never) => Promise<unknown>; onError: (error: unknown) => void }

/** One Rust value's picks. Shared by every component showing it, so a screen
 *  closed and reopened mid-save keeps one queue. */
type Choice = {
  queued: Pick | null
  landed: { value: unknown } | null
  shown: { value: unknown } | null
  saving: boolean
  picks: number
  listeners: Set<() => void>
}

const choices = new Map<string, Choice>()

function choiceOf(id: string): Choice {
  let choice = choices.get(id)
  if (!choice) {
    choice = { queued: null, landed: null, shown: null, saving: false, picks: 0, listeners: new Set() }
    choices.set(id, choice)
  }
  return choice
}

function show(choice: Choice, shown: { value: unknown } | null) {
  choice.shown = shown
  choice.listeners.forEach(listener => listener())
}

/** Saves run one at a time, the newest pick next, so the last to land is the
 *  last Rust wrote. It shows until a read started after it lands. */
async function saveQueued(choice: Choice, queryKey: QueryKey) {
  choice.saving = true
  for (let pick: Pick | null = choice.queued; pick; pick = choice.queued) {
    choice.queued = null
    try {
      await pick.save(pick.value as never)
      choice.landed = { value: pick.value }
    } catch (e) {
      if (!choice.queued) pick.onError(e)
    }
  }
  choice.saving = false
  const picks = choice.picks
  if (choice.landed) {
    show(choice, choice.landed)
    await readAfresh(queryKey)
  }
  if (choice.picks !== picks) return
  choice.landed = null
  show(choice, null)
}

function pick(choice: Choice, queryKey: QueryKey, next: Pick) {
  choice.picks += 1
  choice.queued = next
  show(choice, { value: next.value })
  if (!choice.saving) void saveQueued(choice, queryKey)
}

/**
 * Save a user's pick of the Rust value `queryKey` reads; `part` names it when
 * that query holds several. The pick shows until Rust's value is read back.
 */
export function useLiveChoice<T>(
  queryKey: QueryKey,
  save: (value: T) => Promise<unknown>,
  onError: (error: unknown) => void,
  part = '',
) {
  const choice = choiceOf(hashKey([...queryKey, part]))
  const subscribe = useCallback((listener: () => void) => {
    choice.listeners.add(listener)
    return () => { choice.listeners.delete(listener) }
  }, [choice])
  const pending = useSyncExternalStore(subscribe, () => choice.shown) as { value: T } | null

  const choose = useCallback(
    (value: T) => pick(choice, queryKey, { value, save: save as Pick['save'], onError }),
    [choice, queryKey, save, onError],
  )

  return { pending, choose }
}
