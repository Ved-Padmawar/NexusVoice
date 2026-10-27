import { SpeakerHighIcon } from '@phosphor-icons/react'
import { Switch } from './ui/Switch'
import { useAppStore } from '../store/useAppStore'

/** A short chime once the microphone is live, in either recording mode. */
export function StartSoundToggle() {
  const startSound = useAppStore((s) => s.pill.startSound)
  const updatePill = useAppStore((s) => s.updatePill)

  return (
    <div className="flex items-center gap-3 rounded-2xl bg-panel p-4 ring-1 ring-line-soft ring-inset">
      <span className="grid size-8 place-items-center rounded-[10px] bg-accent-2-soft text-base text-accent-2-ink">
        <SpeakerHighIcon size={16} weight="duotone" />
      </span>
      <div className="flex-1">
        <p className="text-sm font-semibold text-fg">Sound when recording starts</p>
        <p className="text-xs text-pretty text-muted">A short chime once the microphone is listening.</p>
      </div>
      <Switch
        checked={startSound}
        onChange={(on) => { void updatePill({ startSound: on }) }}
        aria-label="Sound when recording starts"
      />
    </div>
  )
}
