import { useSyncExternalStore } from 'react'
import { Channel } from '@tauri-apps/api/core'
import { commands, type Patch } from '../shared/bindings'

/** Each slice Rust pushes, by name. */
export type Slices = { [P in Patch as P['slice']]: P['value'] }
export type SliceName = keyof Slices

const held = new Map<SliceName, { rev: number; value: unknown }>()
const listeners = new Set<() => void>()

/** Keep `patch` unless this window already holds that revision or a newer one. */
function apply(patch: Patch) {
  const current = held.get(patch.slice)
  if (current && current.rev >= patch.rev) return
  held.set(patch.slice, { rev: patch.rev, value: patch.value })
  for (const listener of listeners) listener()
}

/** Mirror Rust's state in this window: the newest of every slice, then each
 *  change in order. Awaited before the first render, so every slice is here
 *  before a component reads it. */
export async function mirrorRust() {
  held.clear()
  const snapshot = await commands.subscribe(new Channel<Patch>(apply))
  snapshot.forEach(apply)
}

function listen(listener: () => void) {
  listeners.add(listener)
  return () => { listeners.delete(listener) }
}

/** `slice` as this window holds it now, for code outside a component. */
export function latest<S extends SliceName>(slice: S): Slices[S] {
  const current = held.get(slice)
  if (!current) throw new Error(`Rust has not sent the ${slice} state`)
  return current.value as Slices[S]
}

/** Part of a slice. `select` returns part of the value, never a new object, so
 *  a component redraws only when that part changes. */
export function useSlice<S extends SliceName, R>(slice: S, select: (value: Slices[S]) => R): R {
  return useSyncExternalStore(listen, () => select(latest(slice)))
}

/** The revision of `slice` this window holds; -1 before Rust sent one. A read
 *  that depends on the slice keys on it. */
export function revisionOf(slice: SliceName): number {
  return held.get(slice)?.rev ?? -1
}

export function useRevision(slice: SliceName): number {
  return useSyncExternalStore(listen, () => revisionOf(slice))
}

/** Resolves once this window holds `slice` at `rev` or a newer revision. */
export function mirrored(slice: SliceName, rev: number) {
  return new Promise<void>((resolve) => {
    const check = () => {
      if ((held.get(slice)?.rev ?? -1) < rev) return
      listeners.delete(check)
      resolve()
    }
    listeners.add(check)
    check()
  })
}
