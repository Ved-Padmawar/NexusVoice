import { useCallback } from 'react'
import { useQuery } from '@tanstack/react-query'
import { toast } from 'sonner'
import { commands, type PillSetting, type RecordingInput } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { useSlice } from '../../app/mirror'
import { reads } from '../../app/sync'
import { useChoice } from '../../app/useChoice'
import { takeLegacyPill } from '../../app/store/legacyPill'
import { useRecordingState } from '../../pill/recordingState'
import { queryKeys } from '../../shared/queryKeys'
import type { HotkeyBindings } from '../../shared/types'

/** Devices come and go with no announcement, so each visit enumerates them again. */
export function useInputDevices() {
  return useQuery({
    queryKey: queryKeys.inputDevices,
    queryFn: () => commands.listInputDevices(),
    ...reads('settings'),
    refetchOnMount: 'always',
  })
}

/** The formatter's saved setup. Keys never leave Rust: profiles carry `hasApiKey`. */
export function useFormatConfig() {
  return useQuery({
    queryKey: queryKeys.formatConfig,
    queryFn: () => commands.getFormatConfig(),
    ...reads('settings'),
  })
}

export function usePasteMode(enabled: boolean) {
  return useQuery({
    queryKey: queryKeys.pasteMode,
    queryFn: () => commands.getPasteMode(),
    enabled,
    ...reads('settings'),
  })
}

export function useKeepMicOpen() {
  return useQuery({
    queryKey: queryKeys.keepMicOpen,
    queryFn: () => commands.getKeepMicOpen(),
    ...reads('settings'),
  })
}

/** True while the recording in progress keeps its previous `input`. */
export function useKeptByRecording(input: RecordingInput) {
  return useRecordingState()?.deferred.includes(input) === true
}

export function useHotkeys<T = HotkeyBindings>(select?: (hotkeys: HotkeyBindings) => T) {
  return useQuery({
    queryKey: queryKeys.hotkeys,
    queryFn: () => commands.getHotkeys(),
    select,
    ...reads('settings'),
  })
}

/** The shortcuts the OS granted, which can be fewer than the saved ones. */
export function useBoundHotkeys() {
  return useQuery({
    queryKey: queryKeys.boundHotkeys,
    queryFn: () => commands.getBoundHotkeys(),
    ...reads('hotkeys'),
  })
}

/** The pill's saved preferences, which the pill window also follows. */
export function usePillSettings() {
  return useSlice('settings', settings => settings.pill)
}

type PillKind = PillSetting['kind']
type PillValue<K extends PillKind> = Extract<PillSetting, { kind: K }>['value']

const savePillSetting = (change: PillSetting) => commands.setPillSetting(change)

const pillSaveFailed = (e: unknown) => toast.error(extractErrorMessage(e, 'Could not save the pill setting'))

/** One pill preference: the pick shows at once, then the saved value. */
export function usePillSetting<K extends PillKind>(kind: K) {
  const saved = useSlice('settings', settings => settings.pill[kind]) as PillValue<K>
  const save = useCallback((value: PillValue<K>) => savePillSetting({ kind, value } as PillSetting), [kind])
  const { pending, choose } = useChoice('settings', `pill.${kind}`, save, pillSaveFailed)
  return [pending ? pending.value : saved, choose] as const
}

/** Hand the pill preferences an older version kept in the browser to Rust, once. */
export async function migrateLegacyPill() {
  const legacy = takeLegacyPill()
  if (!legacy) return
  const changes = Object.entries(legacy).map(([kind, value]) => ({ kind, value }) as PillSetting)
  try {
    for (const change of changes) await savePillSetting(change)
  } catch (e) {
    pillSaveFailed(e)
  }
}
