import { useQuery } from '@tanstack/react-query'
import { toast } from 'sonner'
import {
  commands, events, type PillSettings, type PillSettingsSnapshot, type RecordingInput, type RecordingSnapshot,
} from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { logger } from '../../shared/logger'
import { LIVE, live, patchLive, queryClient, readFresh } from '../../app/queryClient'
import { takeLegacyPill } from '../../app/store/legacyPill'
import { subscribePillSettings } from '../../pill/pillSettings'
import { queryKeys } from '../../shared/queryKeys'
import type { HotkeyBindings, HotkeyKind } from '../../shared/types'

/** Not event-driven: devices come and go, so each visit enumerates them again. */
export const inputDevices = live(queryKeys.inputDevices, () => commands.listInputDevices())

export function useInputDevices() {
  return useQuery({ ...inputDevices, refetchOnMount: 'always' })
}

/** The formatter's saved setup. Keys never leave Rust: profiles carry `hasApiKey`. */
export const formatConfigOptions = live(queryKeys.formatConfig, () => commands.getFormatConfig())

export function useFormatConfig() {
  return useQuery(formatConfigOptions)
}

export const pasteModeOptions = live(queryKeys.pasteMode, () => commands.getPasteMode())

const recordingOptions = live(queryKeys.recording, () => commands.getRecordingState())

/** Keep the recording snapshot current; a revision rejects a late older one. */
export function listenForRecordingState() {
  return events.recordingStateChanged.listen(({ payload }) =>
    patchLive(recordingOptions, (current: RecordingSnapshot) =>
      payload.revision > current.revision ? payload : current))
}

/** True while the recording in progress keeps its previous `input`. */
export function useKeptByRecording(input: RecordingInput) {
  return useQuery({
    ...recordingOptions,
    select: (snapshot: RecordingSnapshot) => snapshot.deferred.includes(input),
  }).data === true
}

const hotkeysOptions = {
  queryKey: queryKeys.hotkeys,
  queryFn: () => commands.getHotkeys(),
  ...LIVE,
}

export function useHotkeys<T = HotkeyBindings>(select?: (hotkeys: HotkeyBindings) => T) {
  return useQuery({ ...hotkeysOptions, select })
}

const boundHotkeysOptions = live(queryKeys.boundHotkeys, () => commands.getBoundHotkeys())

/** The shortcuts the OS granted, which can be fewer than the saved ones. */
export function useBoundHotkeys() {
  return useQuery(boundHotkeysOptions)
}

/** Read what the OS holds again after a binding changed. */
export const refreshBoundHotkeys = () => readFresh(boundHotkeysOptions).catch(() => {})

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
  if (!cachedPill()) await queryClient.query(pillOptions).catch(() => {})
  const saved = cachedPill()
  // Merging into the defaults would save them over the user's other choices.
  if (!saved) {
    toast.error('Could not load the pill settings, so nothing was changed')
    return
  }
  setPill({ ...saved, ...change })
  savesPending += 1
  try {
    isNewest(await commands.setPillSettings({ ...saved, ...change }))
  } catch (e) {
    toast.error(extractErrorMessage(e, 'Could not save the pill setting'))
  }
  savesPending -= 1
  if (savesPending > 0) return
  // Show what Rust holds: the failed edit falls back to the saved value.
  try {
    newest ??= await commands.getPillSettings()
    setPill(newest.settings)
  } catch (e) {
    logger.error('could not read pill settings back', e)
    setPill(saved)
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
