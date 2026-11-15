import { useCallback, useSyncExternalStore } from 'react'
import type { Committed } from '../shared/bindings'
import { mirrored, type SliceName } from './mirror'

type Pick = { value: unknown; save: (value: never) => Promise<Committed>; onError: (error: unknown) => void }

/** One Rust value's picks. Shared by every component showing it, so a screen
 *  closed and reopened mid-save keeps one queue. */
type Choice = {
  queued: Pick | null
  shown: { value: unknown } | null
  saving: boolean
  picks: number
  listeners: Set<() => void>
}

const choices = new Map<string, Choice>()

function choiceOf(id: string): Choice {
  let choice = choices.get(id)
  if (!choice) {
    choice = { queued: null, shown: null, saving: false, picks: 0, listeners: new Set() }
    choices.set(id, choice)
  }
  return choice
}

function show(choice: Choice, shown: { value: unknown } | null) {
  choice.shown = shown
  choice.listeners.forEach(listener => listener())
}

/** Saves run one at a time, the newest pick next, so the last to land is the
 *  last Rust wrote. The pick shows until the mirror holds what it committed. */
async function saveQueued(choice: Choice, slice: SliceName) {
  choice.saving = true
  let committed: Committed | null = null
  for (let pick: Pick | null = choice.queued; pick; pick = choice.queued) {
    choice.queued = null
    try {
      committed = await pick.save(pick.value as never)
    } catch (e) {
      if (!choice.queued) pick.onError(e)
    }
  }
  choice.saving = false
  const picks = choice.picks
  if (committed) await mirrored(slice, committed.rev)
  if (choice.picks === picks) show(choice, null)
}

function pick(choice: Choice, slice: SliceName, next: Pick) {
  choice.picks += 1
  choice.queued = next
  show(choice, { value: next.value })
  if (!choice.saving) void saveQueued(choice, slice)
}

/** Save a user's pick of the Rust value `id` in `slice`. A refused pick shows
 *  the saved value again. */
export function useChoice<T>(
  slice: SliceName,
  id: string,
  save: (value: T) => Promise<Committed>,
  onError: (error: unknown) => void,
) {
  const choice = choiceOf(`${slice}:${id}`)
  const subscribe = useCallback((listener: () => void) => {
    choice.listeners.add(listener)
    return () => { choice.listeners.delete(listener) }
  }, [choice])
  const pending = useSyncExternalStore(subscribe, () => choice.shown) as { value: T } | null

  const choose = useCallback(
    (value: T) => pick(choice, slice, { value, save: save as Pick['save'], onError }),
    [choice, slice, save, onError],
  )

  return { pending, choose }
}
