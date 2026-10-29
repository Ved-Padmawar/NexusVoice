import { useQuery } from '@tanstack/react-query'
import { commands, type PillSettings, type PillSettingsSnapshot } from '../../shared/bindings'
import { logger } from '../../shared/logger'
import { LIVE, queryClient } from '../../app/queryClient'
import { takeLegacyPill } from '../../app/store/legacyPill'
import { subscribePillSettings } from '../../pill/pillSettings'
import { queryKeys } from '../../shared/queryKeys'
import type { HotkeyBindings, HotkeyKind } from '../../shared/types'

const hotkeysOptions = {
  queryKey: queryKeys.hotkeys,
  queryFn: () => commands.getHotkeys(),
  ...LIVE,
}

export function useHotkeys<T = HotkeyBindings>(select?: (hotkeys: HotkeyBindings) => T) {
  return useQuery({ ...hotkeysOptions, select })
}

/** Record a binding Rust just accepted. */
export const setCachedHotkey = (kind: HotkeyKind, hotkey: string | null) =>
  queryClient.setQueryData<HotkeyBindings>(queryKeys.hotkeys, hotkeys => hotkeys && { ...hotkeys, [kind]: hotkey })

const DEFAULT_PILL: PillSettings = { theme: 'steel', waveformStyle: 'bars', liveTranscript: false, startSound: false }

let savesPending = 0
let newest: PillSettingsSnapshot | undefined

// Only a snapshot newer than any seen may apply, so a delayed event never
// replays an older edit.
const isNewest = (snapshot: PillSettingsSnapshot) => {
  if (newest && snapshot.revision <= newest.revision) return false
  newest = snapshot
  return true
}

const cachedPill = () => queryClient.getQueryData<PillSettings>(queryKeys.pillSettings)
const setPill = (settings: PillSettings) => queryClient.setQueryData(queryKeys.pillSettings, settings)

const pillOptions = {
  queryKey: queryKeys.pillSettings,
  queryFn: async () => {
    const snapshot = await commands.getPillSettings()
    isNewest(snapshot)
    // A pending edit is newer than anything this read could return.
    return (savesPending > 0 && cachedPill()) || (newest ?? snapshot).settings
  },
  ...LIVE,
}

/** The pill's saved preferences, which the pill window also follows. */
export function usePillSettings() {
  return useQuery(pillOptions).data ?? DEFAULT_PILL
}

// Applied at once so the picker responds; snapshots wait out pending saves.
export async function updatePill(change: Partial<PillSettings>) {
  // Merging into the defaults would save them over the user's other choices.
  if (!cachedPill()) await queryClient.query(pillOptions).catch(() => {})
  const next = { ...(cachedPill() ?? DEFAULT_PILL), ...change }
  setPill(next)
  savesPending += 1
  try {
    isNewest(await commands.setPillSettings(next))
  } catch (e) {
    logger.error('could not save pill settings', e)
  }
  savesPending -= 1
  if (savesPending === 0) {
    newest ??= await commands.getPillSettings()
    setPill(newest.settings)
  }
}

export function followPillSettings() {
  const stop = subscribePillSettings((settings, revision) => {
    if (isNewest({ revision, settings }) && savesPending === 0) setPill(settings)
  })
  const legacy = takeLegacyPill()
  if (legacy) void updatePill(legacy)
  return stop
}
