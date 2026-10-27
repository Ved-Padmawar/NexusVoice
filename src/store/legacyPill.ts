import type { PillSettings } from '../bindings'

/** Pill preferences that persisted-store version 1 kept; handed to the backend once. */
let legacyPill: Partial<PillSettings> | null = null

export function stashLegacyPill(pill: Partial<PillSettings>) {
  legacyPill = pill
}

export function takeLegacyPill(): Partial<PillSettings> | null {
  const pill = legacyPill
  legacyPill = null
  return pill
}
