import { useCallback } from 'react'
import { useQuery } from '@tanstack/react-query'
import { toast } from 'sonner'
import { commands, type Committed, type PillSetting, type RecordingInput, type SettingsView } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { useSlice } from '../../app/mirror'
import { useChoice } from '../../app/useChoice'
import { takeLegacyPill } from '../../app/store/legacyPill'
import { useRecordingState } from '../../pill/recordingState'
import { queryKeys } from '../../shared/queryKeys'

/** Devices come and go with no announcement, so each visit enumerates them again. */
export function useInputDevices() {
  return useQuery({
    queryKey: queryKeys.inputDevices,
    queryFn: () => commands.listInputDevices(),
    refetchOnMount: 'always',
  })
}

/** A saved preference, and how to change it: a pick shows until it is saved. */
export function useSetting<K extends keyof SettingsView>(
  key: K,
  save: (value: SettingsView[K]) => Promise<Committed>,
  onError: (error: unknown) => void,
) {
  const saved = useSlice('settings', settings => settings[key])
  const { pending, choose } = useChoice('settings', key, save, onError)
  return [pending ? pending.value : saved, choose] as const
}

/** True while the recording in progress keeps its previous `input`. */
export function useKeptByRecording(input: RecordingInput) {
  return useRecordingState()?.deferred.includes(input) === true
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
