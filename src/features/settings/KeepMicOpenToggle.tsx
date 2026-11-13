import { useEffect, useState } from 'react'
import { LightningIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { Switch } from '../../shared/ui/Switch'
import { SettingRow } from './SettingRow'

export function KeepMicOpenToggle() {
  const [keepOpen, setKeepOpen] = useState(false)
  // Locked until the first read and during a save: it shows only what Rust confirmed.
  const [busy, setBusy] = useState(true)

  useEffect(() => {
    commands.getKeepMicOpen()
      .then(setKeepOpen)
      .catch((e) => toast.error(extractErrorMessage(e, 'Could not read the microphone setting')))
      .finally(() => setBusy(false))
  }, [])

  const change = async (keep: boolean) => {
    setBusy(true)
    try {
      await commands.setKeepMicOpen(keep)
      setKeepOpen(keep)
    } catch (e) {
      toast.error(extractErrorMessage(e, 'Could not save the microphone setting'))
    } finally {
      setBusy(false)
    }
  }

  return (
    <SettingRow
      icon={<LightningIcon size={16} weight="duotone" />}
      title="Keep the microphone ready"
      tip="Keeps the microphone open between recordings, so your system's microphone indicator stays on. Off, it closes after 60 seconds without a recording."
      description={<p className="text-xs text-muted">The first word is never clipped.</p>}
    >
      <Switch checked={keepOpen} disabled={busy} onChange={(on) => { void change(on) }} aria-label="Keep the microphone ready" />
    </SettingRow>
  )
}
