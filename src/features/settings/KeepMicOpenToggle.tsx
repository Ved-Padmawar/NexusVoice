import { LightningIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { Switch } from '../../shared/ui/Switch'
import { SettingRow } from './SettingRow'
import { useSetting } from './queries'

const saveKeepOpen = (keep: boolean) => commands.setKeepMicOpen(keep)
const saveFailed = (e: unknown) => toast.error(extractErrorMessage(e, 'Could not save the microphone setting'))

export function KeepMicOpenToggle() {
  const [keepOpen, choose] = useSetting('keepMicOpen', saveKeepOpen, saveFailed)

  return (
    <SettingRow
      icon={<LightningIcon size={16} weight="duotone" />}
      title="Keep the microphone ready"
      tip="Keeps the microphone open between recordings, so your system's microphone indicator stays on. Off, it closes after 60 seconds without a recording."
      description={<p className="text-xs text-muted">The first word is never clipped.</p>}
    >
      <Switch
        checked={keepOpen}
        onChange={choose}
        aria-label="Keep the microphone ready"
      />
    </SettingRow>
  )
}
