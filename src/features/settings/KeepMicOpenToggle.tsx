import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { Switch } from '../../shared/ui/Switch'

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
    <label
      data-tip="Turn this on to keep the microphone open between recordings. Your system's microphone indicator stays on while it is. Off, the microphone closes after 60 seconds of no recording."
      data-tip-align="end"
      className="flex shrink-0 cursor-pointer items-center gap-2 text-xs font-semibold text-fg-2"
    >
      Keep ready
      <Switch checked={keepOpen} disabled={busy} onChange={(on) => { void change(on) }} aria-label="Keep the microphone ready" />
    </label>
  )
}
