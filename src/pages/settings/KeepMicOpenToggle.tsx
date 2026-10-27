import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { commands } from '../../bindings'
import { extractErrorMessage } from '../../lib/errors'
import { Switch } from '../../components/ui/Switch'

export function KeepMicOpenToggle() {
  const [keepOpen, setKeepOpen] = useState(false)

  useEffect(() => {
    commands.getKeepMicOpen()
      .then(setKeepOpen)
      .catch((e) => toast.error(extractErrorMessage(e, 'Could not read the microphone setting')))
  }, [])

  const change = async (keep: boolean) => {
    setKeepOpen(keep)
    try {
      await commands.setKeepMicOpen(keep)
    } catch (e) {
      setKeepOpen(!keep)
      toast.error(extractErrorMessage(e, 'Could not save the microphone setting'))
    }
  }

  return (
    <label
      data-tip="Turn this on to keep the microphone open between recordings. Your system's microphone indicator stays on while it is. Off, the microphone closes after 60 seconds of no recording."
      data-tip-align="end"
      className="flex shrink-0 cursor-pointer items-center gap-2 text-xs font-semibold text-fg-2"
    >
      Keep ready
      <Switch checked={keepOpen} onChange={(on) => { void change(on) }} aria-label="Keep the microphone ready" />
    </label>
  )
}
