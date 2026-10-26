import { useState, useEffect, useCallback, memo } from 'react'
import { MicrophoneIcon, ArrowsClockwiseIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands } from '../../bindings'
import { extractErrorMessage } from '../../lib/errors'
import { Select } from '../../components/ui/Select'
import type { InputDevice } from '../../types'

/** Sentinel for the "Default" option — maps to no saved preference. */
const DEFAULT_VALUE = '__default__'

export const MicrophoneSection = memo(function MicrophoneSection() {
  const [devices, setDevices] = useState<InputDevice[]>([])
  const [selected, setSelected] = useState<string>(DEFAULT_VALUE)
  const [initialLoading, setInitialLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)

  const fetchDevices = useCallback(async () => {
    const devices = await commands.listInputDevices()
    setDevices(devices)
    const active = devices.find(d => d.isSelected && !d.isDefault)
    setSelected(active ? active.name : DEFAULT_VALUE)
  }, [])

  useEffect(() => {
    fetchDevices()
      .catch(e => toast.error(extractErrorMessage(e, 'Could not list microphones')))
      .finally(() => setInitialLoading(false))
  }, [fetchDevices])

  // Refresh keeps the controls interactive; only the icon spins. A minimum
  // duration guarantees the spin is visible even when the device list is cached.
  const refresh = useCallback(() => {
    if (refreshing) return
    setRefreshing(true)
    Promise.all([
      fetchDevices().catch(e => toast.error(extractErrorMessage(e, 'Could not list microphones'))),
      new Promise(r => setTimeout(r, 500)),
    ]).finally(() => setRefreshing(false))
  }, [refreshing, fetchDevices])

  const choose = useCallback(async (value: string) => {
    if (value === selected) return
    const previous = selected
    setSelected(value)
    try {
      await commands.setInputDevice(value === DEFAULT_VALUE ? null : value)
    } catch (e) {
      setSelected(previous)
      toast.error(extractErrorMessage(e, 'Could not set microphone'))
    }
  }, [selected])

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
      <div>
        <p className="text-sm font-semibold text-fg">Microphone</p>
        <p className="text-xs text-muted">The device that records your voice.</p>
      </div>

      <div className="flex gap-2">
        <div className="min-w-0 flex-1">
          <Select
            value={selected}
            options={options}
            onChange={(v) => void choose(v)}
            aria-label="Microphone"
            icon={<MicrophoneIcon size={14} weight="bold" />}
            disabled={initialLoading}
          />
        </div>
        <button
          type="button"
          onClick={refresh}
          disabled={initialLoading}
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
