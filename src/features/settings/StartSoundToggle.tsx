import { SpeakerHighIcon } from '@phosphor-icons/react'
import { Switch } from '../../shared/ui/Switch'
import { SettingRow } from './SettingRow'
import { updatePill, usePillSettings } from './queries'

/** A short chime once the microphone is live, in either recording mode. */
export function StartSoundToggle() {
  const { startSound } = usePillSettings()

  return (
    <SettingRow
      icon={<SpeakerHighIcon size={16} weight="duotone" />}
      title="Sound when recording starts"
      description={<p className="text-xs text-pretty text-muted">A short chime once the microphone is listening.</p>}
    >
      <Switch
        checked={startSound}
        onChange={(on) => { void updatePill({ startSound: on }) }}
        aria-label="Sound when recording starts"
      />
    </SettingRow>
  )
}
