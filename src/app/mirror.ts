import { useSyncExternalStore } from 'react'
import { Channel } from '@tauri-apps/api/core'
import { commands, type Change, type Patch, type SliceName } from '../shared/bindings'

/** Each slice Rust pushes, by name. */
export type Slices = { [P in Patch as P['slice']]: P['value'] }
export type { SliceName }

/** Every slice, for a window that shows them all; the record type fails the
 *  build when Rust adds a slice this misses. */
export const ALL_SLICES = Object.keys({
  settings: true, hotkeys: true, models: true, downloads: true, cuda: true, recording: true,
  jobs: true, dictionary: true, savedAudio: true, history: true, historyTotals: true,
} satisfies Record<SliceName, true>) as SliceName[]

const held = new Map<SliceName, { rev: number; value: unknown }>()
const listeners = new Set<() => void>()
/** The newest revision this window received. */
let windowRev = -1
/** Each slice's revision when the window was hidden; null while it is shown. */
let revsWhenHidden: Map<SliceName, number> | null = null

/** Keep each patch of `change` this window doesn't already hold newer, then
 *  tell every listener once, so a commit that changed several slices shows
 *  as one. */
function apply(change: Change) {
  for (const patch of change.patches) {
    const current = held.get(patch.slice)
    if (!current || current.rev < change.rev) held.set(patch.slice, { rev: change.rev, value: patch.value })
  }
  windowRev = Math.max(windowRev, change.rev)
  for (const listener of listeners) listener()
}

/** Mirror `slices` of Rust's state in this window: their newest values, then
 *  every commit in order. Awaited before the first render, so each slice is
 *  here before a component reads it. */
export async function mirrorRust(slices: SliceName[]) {
  held.clear()
  windowRev = -1
  apply(await commands.subscribe(slices, new Channel<Change>(apply)))
}

function listen(listener: () => void) {
  listeners.add(listener)
  return () => { listeners.delete(listener) }
}

/** `slice` as this window holds it now, for code outside a component. */
export function latest<S extends SliceName>(slice: S): Slices[S] {
  const current = held.get(slice)
  if (!current) throw new Error(`this window does not mirror the ${slice} state`)
  return current.value as Slices[S]
}

/** Part of a slice. `select` returns part of the value, never a new object, so
 *  a component redraws only when that part changes. */
export function useSlice<S extends SliceName, R>(slice: S, select: (value: Slices[S]) => R): R {
  return useSyncExternalStore(listen, () => select(latest(slice)))
}

/** The revision `slice` last changed at; -1 before Rust sent it. */
export function revisionOf(slice: SliceName): number {
  return held.get(slice)?.rev ?? -1
}

/** The revision a read of `slice` keys on. A window hidden to the tray keeps
 *  the one it had, so it reads nothing until it is shown again. */
export function useRevision(slice: SliceName): number {
  return useSyncExternalStore(listen, () => revsWhenHidden?.get(slice) ?? revisionOf(slice))
}

/** Hiding to the tray leaves `document.visibilityState` at `visible`, so the
 *  window says when it is shown. */
export function setWindowShown(shown: boolean) {
  if (shown === !revsWhenHidden) return
  revsWhenHidden = shown ? null : new Map([...held].map(([slice, { rev }]) => [slice, rev]))
  for (const listener of listeners) listener()
}

/** Resolves once this window holds revision `rev`: every window receives
 *  every revision. */
export function mirrored(rev: number) {
  return new Promise<void>((resolve) => {
    const check = () => {
      if (windowRev < rev) return
      listeners.delete(check)
      resolve()
    }
    listeners.add(check)
    check()
  })
}
