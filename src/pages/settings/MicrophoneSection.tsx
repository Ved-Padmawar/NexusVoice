import { useState, useEffect, useCallback, memo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Mic, RefreshCw } from 'lucide-react'
import { toast } from 'sonner'
import { COMMANDS } from '../../lib/commands'
import { extractErrorMessage } from '../../lib/errors'
import { IconButton } from '@/components/ui/button'
import { SelectMenu, SelectMenuItem } from '@/components/ui/select-menu'
import { SettingRow } from '../../components/page'

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
  const defaultOption = defaultLabel ? `Default — ${defaultLabel}` : 'Default'
  const currentLabel = selected === DEFAULT_VALUE ? defaultOption : selected

  // The default device is represented by the "Default — <name>" sentinel, so
  // list only the non-default devices by name (deduped) to avoid a duplicate row.
  const seen = new Set<string>()
  const options = [
    { value: DEFAULT_VALUE, label: defaultOption },
    ...devices
      .filter(d => !d.isDefault && !d.name.startsWith('Default') && (seen.has(d.name) ? false : seen.add(d.name)))
      .map(d => ({ value: d.name, label: d.name })),
  ]

  return (
    <SettingRow title="Microphone" description="The input device that records your voice." field>
      <div className="min-w-0 flex-1">
        <SelectMenu
          value={selected}
          onValueChange={(v) => void choose(v)}
          open={open}
          onOpenChange={setOpen}
          disabled={initialLoading}
          icon={<Mic strokeWidth={2} />}
          label={currentLabel}
          display={currentLabel}
        >
          {options.map(opt => (
            <SelectMenuItem key={opt.value} value={opt.value}>{opt.label}</SelectMenuItem>
          ))}
        </SelectMenu>
      </div>
      <IconButton label="Refresh device list" framed onClick={refresh} disabled={initialLoading}>
        <RefreshCw strokeWidth={1.9} className={refreshing ? 'nv-spin' : undefined} />
      </IconButton>
    </SettingRow>
  )
})
