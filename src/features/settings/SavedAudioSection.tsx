import { useState } from 'react'
import { MinusIcon, PlusIcon, TrashIcon, WaveformIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { cn, formatBytes } from '../../shared/utils'
import { useSlice } from '../../app/mirror'
import { useChoice } from '../../app/useChoice'
import { Switch } from '../../shared/ui/Switch'
import { Button } from '../../shared/ui/Button'
import { useImporting } from '../history/jobs'
import { SettingRow, SettingsCard } from './SettingRow'

const MIN_KEEP = 1
const MAX_KEEP = 100

const saveEnabled = (value: boolean) => commands.setSavedAudio({ kind: 'enabled', value })
const saveKeep = (value: number) => commands.setSavedAudio({ kind: 'keep', value })
const saveFailed = (e: unknown) => toast.error(extractErrorMessage(e, 'Could not save the audio setting'))

const stepper = 'grid size-7 cursor-pointer place-items-center text-fg-2 hover:text-fg disabled:cursor-default disabled:opacity-40 disabled:hover:text-fg-2'

/** Whether History entries keep their audio for replay and re-transcribing, and for how many. */
export function SavedAudioSection() {
  const saved = useSlice('settings', settings => settings.savedAudio)
  const enabledPick = useChoice('settings', 'savedAudio.enabled', saveEnabled, saveFailed)
  const keepPick = useChoice('settings', 'savedAudio.keep', saveKeep, saveFailed)
  // Mirrors Rust, which won't clear while imported files still need their audio.
  const importing = useImporting()

  const shown = {
    enabled: enabledPick.pending?.value ?? saved.enabled,
    keep: keepPick.pending?.value ?? saved.keep,
  }
  const { bytes: remaining, cleanupFailed } = useSlice('savedAudio', status => status)
  const [retrying, setRetrying] = useState(false)

  const retry = async () => {
    setRetrying(true)
    try {
      await commands.retrySavedAudioCleanup()
    } catch (e) {
      toast.error(extractErrorMessage(e, 'Could not finish the saved audio cleanup'))
    } finally {
      setRetrying(false)
    }
  }

  const clear = () => {
    commands.clearSavedAudio()
      .then(({ deleted, kept }) => {
        if (!deleted) toast.info(kept ? 'Pinned entries keep their audio. Unpin one to delete it.' : 'There is no saved audio to delete.')
        else toast.success(kept ? 'Saved audio deleted. Pinned entries keep theirs.' : 'Saved audio deleted')
      })
      .catch((e: unknown) => toast.error(extractErrorMessage(e, 'Could not delete the saved audio')))
  }

  return (
    <SettingsCard>
      <SettingRow
        icon={<WaveformIcon size={16} weight="duotone" />}
        title="Saved audio"
        tip="Lets you replay or re-transcribe an entry from History. Dictations and imported files share the limit. Stored as WAV, about 1.9 MB a minute. Pinned entries always keep theirs, and a file still being transcribed keeps its audio until it finishes, even with this off."
        description={
          <p className="text-xs text-muted tabular-nums">
            {shown.enabled ? `Last ${shown.keep} entries · ${formatBytes(remaining)}` : remaining ? `Off · ${formatBytes(remaining)} still saved` : 'Off'}
          </p>
        }
        below={cleanupFailed && (
          <div role="status" className="flex items-center justify-between gap-2 text-xs text-muted">
            <p>Your audio setting is saved. Some recordings couldn't be removed.</p>
            <Button size="sm" variant="outline" disabled={retrying} onClick={() => void retry()}>Retry cleanup</Button>
          </div>
        )}
      >
        <span
          className={cn('inline-flex shrink-0 items-center rounded-lg bg-field ring-1 ring-line-soft transition-opacity ring-inset', !shown.enabled && 'pointer-events-none opacity-40')}
          data-tip="How many recent entries keep their audio"
          data-tip-align="end"
        >
          <button type="button" aria-label="Fewer" disabled={shown.keep <= MIN_KEEP} onClick={() => keepPick.choose(shown.keep - 1)} className={stepper}>
            <MinusIcon size={11} weight="bold" />
          </button>
          <b className="w-7 text-center text-sm font-semibold text-fg tabular-nums">{shown.keep}</b>
          <button type="button" aria-label="More" disabled={shown.keep >= MAX_KEEP} onClick={() => keepPick.choose(shown.keep + 1)} className={stepper}>
            <PlusIcon size={11} weight="bold" />
          </button>
        </span>
        <button
          type="button"
          aria-label="Delete recordings"
          data-tip={importing ? 'Wait for the files being transcribed' : 'Delete recordings'}
          data-tip-align="end"
          disabled={importing}
          onClick={clear}
          className={cn('grid size-7 shrink-0 cursor-pointer place-items-center rounded-lg text-muted hover:bg-danger-soft hover:text-danger disabled:pointer-events-none disabled:opacity-40', !shown.enabled && !remaining && 'invisible')}
        >
          <TrashIcon size={14} />
        </button>
        <Switch
          aria-label="Saved audio"
          checked={shown.enabled}
          onChange={enabledPick.choose}
        />
      </SettingRow>
    </SettingsCard>
  )
}
