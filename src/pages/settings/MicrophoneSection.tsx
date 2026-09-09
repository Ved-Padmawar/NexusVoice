import { useState, useEffect, useCallback, memo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Select } from 'radix-ui'
import { Mic, RefreshCw, Check, ChevronDown } from 'lucide-react'
import { toast } from 'sonner'
import { COMMANDS } from '../../lib/commands'
import { extractErrorMessage } from '../../lib/errors'
import { SELECT_CONTENT, SELECT_ITEM, SELECT_TRIGGER } from './selectStyles'

type InputDevice = {
  name: string
  isDefault: boolean
  isSelected: boolean
}

/** Sentinel for the "Default" option — maps to no saved preference. */
const DEFAULT_VALUE = '__default__'

export const MicrophoneSection = memo(function MicrophoneSection() {
  const [devices, setDevices] = useState<InputDevice[]>([])
  const [selected, setSelected] = useState<string>(DEFAULT_VALUE)
  const [initialLoading, setInitialLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [open, setOpen] = useState(false)

  const fetchDevices = useCallback(async () => {
    const list = await invoke<InputDevice[]>(COMMANDS.LIST_INPUT_DEVICES)
    const devices = Array.isArray(list) ? list : []
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
    setOpen(false)
    if (value === selected) return
    const previous = selected
    setSelected(value)
    try {
      await invoke<void>(COMMANDS.SET_INPUT_DEVICE, {
        name: value === DEFAULT_VALUE ? null : value,
      })
    } catch (e) {
      setSelected(previous)
      toast.error(extractErrorMessage(e, 'Could not set microphone'))
    }
  }, [selected])

  const defaultLabel = devices.find(d => d.isDefault)?.name
  const currentLabel = selected === DEFAULT_VALUE
    ? (defaultLabel ? `Default — ${defaultLabel}` : 'Default')
    : selected

  // The default device is represented by the "Default — <name>" sentinel, so
  // list only the non-default devices by name (deduped) to avoid a duplicate row.
  const seen = new Set<string>()
  const options = [
    { value: DEFAULT_VALUE, label: defaultLabel ? `Default — ${defaultLabel}` : 'Default' },
    ...devices
      .filter(d => !d.isDefault && !d.name.startsWith('Default') && (seen.has(d.name) ? false : seen.add(d.name)))
      .map(d => ({ value: d.name, label: d.name })),
  ]

  return (
    <div className="flex min-w-0 max-w-96 flex-col gap-2">
      <span className="text-[11px] text-(--muted)">Microphone</span>

      <div className="flex min-w-0 items-center gap-2">
        <Select.Root
          value={selected}
          onValueChange={(v) => void choose(v)}
          open={open}
          onOpenChange={setOpen}
        >
          <Select.Trigger asChild disabled={initialLoading}>
            <button type="button" aria-label={currentLabel} className={`${SELECT_TRIGGER} w-auto min-w-0 flex-1`}>
              <Mic size={13} strokeWidth={2} className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-(--on-soft)" />
              <span className="truncate">
                <Select.Value>{currentLabel}</Select.Value>
              </span>
              <ChevronDown
                size={13}
                strokeWidth={2}
                className="pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 text-(--muted) transition-transform duration-(--t-fast) group-data-[state=open]:rotate-180"
              />
            </button>
          </Select.Trigger>

          <Select.Portal>
            <Select.Content position="popper" sideOffset={5} className={SELECT_CONTENT}>
              <Select.Viewport
                className="select-list"
                style={{ overflowY: 'auto', overscrollBehavior: 'none', maxHeight: '16rem' }}
              >
                {options.map((opt) => (
                  <Select.Item key={opt.value} value={opt.value} className={SELECT_ITEM}>
                    <span className="min-w-0 flex-1 truncate">
                      <Select.ItemText>{opt.label}</Select.ItemText>
                    </span>
                    <Select.ItemIndicator className="ml-2 shrink-0 text-(--on-soft)">
                      <Check size={13} strokeWidth={2.5} />
                    </Select.ItemIndicator>
                  </Select.Item>
                ))}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>

        <button
          type="button"
          onClick={refresh}
          disabled={initialLoading}
          title="Refresh device list"
          aria-label="Refresh device list"
          className="btn btn-quiet size-8 shrink-0 px-0"
        >
          <RefreshCw size={13} strokeWidth={1.9} className={refreshing ? 'animate-spin' : undefined} />
        </button>
      </div>
    </div>
  )
})
