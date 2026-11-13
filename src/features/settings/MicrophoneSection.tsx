import { useState, useEffect, useCallback, memo } from 'react'
import { MicrophoneIcon, ArrowsClockwiseIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { readFresh } from '../../app/queryClient'
import { useLiveChoice } from '../../app/useLiveChoice'
import { Select } from '../../shared/ui/Select'
import { SettingRow } from './SettingRow'
import { InputHint } from './InputHint'
import { inputDevices, useInputDevices } from './queries'
import type { InputDevice } from '../../shared/types'

/** Sentinel for the "Default" option — maps to no saved preference. */
const DEFAULT_VALUE = '__default__'

const NO_DEVICES: InputDevice[] = []

const listFailed = (e: unknown) => toast.error(extractErrorMessage(e, 'Could not list microphones'))
const saveDevice = (value: string) => commands.setInputDevice(value === DEFAULT_VALUE ? null : value)
const saveFailed = (e: unknown) => toast.error(extractErrorMessage(e, 'Could not set microphone'))

/** Numbers identical mics so each can be told apart. */
function labelled(devices: InputDevice[]) {
  const shared = new Set(devices.filter((d, i) => devices.findIndex(o => o.name === d.name) !== i).map(d => d.name))
  const count = new Map<string, number>()
  return devices.map(d => {
    const n = (count.get(d.name) ?? 0) + 1
    count.set(d.name, n)
    return { ...d, label: shared.has(d.name) ? `${d.name} (${n})` : d.name }
  })
}

export const MicrophoneSection = memo(function MicrophoneSection() {
  const devicesQuery = useInputDevices()
  const devices = devicesQuery.data ?? NO_DEVICES
  const [refreshing, setRefreshing] = useState(false)
  const { pending, choose } = useLiveChoice(inputDevices, saveDevice, saveFailed)

  const saved = devices.find(d => d.isSelected && !d.isDefault)?.id ?? DEFAULT_VALUE
  const selected = pending ? pending.value : saved

  useEffect(() => {
    if (devicesQuery.error) listFailed(devicesQuery.error)
  }, [devicesQuery.error])

  // Refresh keeps the controls interactive; only the icon spins. A minimum
  // duration guarantees the spin is visible even when the device list is cached.
  const refresh = useCallback(() => {
    if (refreshing) return
    setRefreshing(true)
    Promise.all([
      readFresh(inputDevices).catch(listFailed),
      new Promise(r => setTimeout(r, 500)),
    ]).finally(() => setRefreshing(false))
  }, [refreshing])

  const listed = labelled(devices)
  const defaultLabel = listed.find(d => d.isDefault)?.label

  // The default device is represented by the "Default - <name>" sentinel, so
  // it gets no row of its own.
  const options = [
    { value: DEFAULT_VALUE, label: defaultLabel ? `Default - ${defaultLabel}` : 'Default', dividerAfter: true },
    ...listed
      .filter(d => !d.isDefault && !d.name.startsWith('Default'))
      .map(d => ({ value: d.id, label: d.label })),
  ]

  return (
    <SettingRow
      icon={<MicrophoneIcon size={16} weight="duotone" />}
      title="Microphone"
      description={<InputHint input="microphone">The device that records your voice.</InputHint>}
    >
      <div className="flex min-w-0 flex-1 gap-1">
        <div className="min-w-0 flex-1">
          <Select
            value={selected}
            options={options}
            onChange={(v) => { if (v !== selected) void choose(v) }}
            aria-label="Microphone"
            disabled={devicesQuery.isPending}
          />
        </div>
        <button
          type="button"
          onClick={refresh}
          disabled={devicesQuery.isPending}
          aria-label="Refresh the device list"
          data-tip="Refresh the device list"
          data-tip-pos="bottom"
          className="grid size-9 shrink-0 cursor-pointer place-items-center rounded-lg text-fg-2 transition hover:bg-surface-hover hover:text-fg disabled:opacity-50"
        >
          <ArrowsClockwiseIcon size={14} weight="bold" className={refreshing ? 'animate-spin' : undefined} />
        </button>
      </div>
    </SettingRow>
  )
})
