import { useState, useEffect, useCallback, memo } from 'react'
import { MicrophoneIcon, ArrowsClockwiseIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { readFresh } from '../../app/queryClient'
import { useLiveChoice } from '../../app/useLiveChoice'
import { Select } from '../../shared/ui/Select'
import { KeepMicOpenToggle } from './KeepMicOpenToggle'
import { InputHint } from './InputHint'
import { inputDevices, useInputDevices } from './queries'
import type { InputDevice } from '../../shared/types'

/** Sentinel for the "Default" option — maps to no saved preference. */
const DEFAULT_VALUE = '__default__'

const NO_DEVICES: InputDevice[] = []

const listFailed = (e: unknown) => toast.error(extractErrorMessage(e, 'Could not list microphones'))
const saveDevice = (value: string) => commands.setInputDevice(value === DEFAULT_VALUE ? null : value)
const saveFailed = (e: unknown) => toast.error(extractErrorMessage(e, 'Could not set microphone'))

export const MicrophoneSection = memo(function MicrophoneSection() {
  const devicesQuery = useInputDevices()
  const devices = devicesQuery.data ?? NO_DEVICES
  const [refreshing, setRefreshing] = useState(false)
  const { pending, choose } = useLiveChoice(inputDevices, saveDevice, saveFailed)

  const saved = devices.find(d => d.isSelected && !d.isDefault)?.name ?? DEFAULT_VALUE
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

  const defaultLabel = devices.find(d => d.isDefault)?.name

  // The default device is represented by the "Default — <name>" sentinel, so
  // list only the non-default devices by name (deduped) to avoid a duplicate row.
  const seen = new Set<string>()
  const options = [
    { value: DEFAULT_VALUE, label: defaultLabel ? `Default — ${defaultLabel}` : 'Default', dividerAfter: true },
    ...devices
      .filter(d => !d.isDefault && !d.name.startsWith('Default') && (seen.has(d.name) ? false : seen.add(d.name)))
      .map(d => ({ value: d.name, label: d.name })),
  ]

  return (
    <div className="flex min-w-0 flex-col gap-2.5">
      <div className="flex items-center justify-between gap-3">
        <div>
          <p className="text-sm font-semibold text-fg">Microphone</p>
          <InputHint input="microphone">The device that records your voice.</InputHint>
        </div>
        <KeepMicOpenToggle />
      </div>

      <div className="flex gap-2">
        <div className="min-w-0 flex-1">
          <Select
            value={selected}
            options={options}
            onChange={(v) => { if (v !== selected) void choose(v) }}
            aria-label="Microphone"
            icon={<MicrophoneIcon size={14} weight="bold" />}
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
          className="grid size-9 shrink-0 cursor-pointer place-items-center rounded-lg bg-field text-fg-2 ring-1 ring-line-soft ring-inset transition hover:text-fg hover:ring-line disabled:opacity-50"
        >
          <ArrowsClockwiseIcon size={14} weight="bold" className={refreshing ? 'animate-spin' : undefined} />
        </button>
      </div>
    </div>
  )
})
