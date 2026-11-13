import { useEffect } from 'react'
import { MinusIcon, PlusIcon, TrashIcon, WaveformIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { cn } from '../../shared/utils'
import { useLiveChoice } from '../../app/useLiveChoice'
import { Switch } from '../../shared/ui/Switch'
import { savedAudioOptions, useImporting, useSavedAudio } from '../history/jobs'
import { SettingRow, SettingsCard } from './SettingRow'

const MIN_KEEP = 1
const MAX_KEEP = 100

type Choice = { enabled: boolean; keep: number }

const save = ({ enabled, keep }: Choice) => commands.setSavedAudio(enabled, keep)
const saveFailed = (e: unknown) => toast.error(extractErrorMessage(e, 'Could not save the audio setting'))

const fmtBytes = (bytes: number) =>
  bytes >= 1_048_576 ? `${(bytes / 1_048_576).toFixed(1)} MB` : `${Math.round(bytes / 1024)} KB`

const stepper = 'grid size-7 cursor-pointer place-items-center text-fg-2 hover:text-fg disabled:cursor-default disabled:opacity-40 disabled:hover:text-fg-2'

/** Whether History entries keep their audio for replay and re-transcribing, and for how many. */
export function SavedAudioSection() {
  const { data, error } = useSavedAudio()
  const { pending, choose } = useLiveChoice(savedAudioOptions, save, saveFailed)
  // Mirrors Rust, which won't clear while imported files still need their audio.
  const importing = useImporting()

  useEffect(() => {
    if (error) toast.error(extractErrorMessage(error, 'Could not load the audio setting'))
  }, [error])

  const shown = pending?.value ?? (data ? { enabled: data.enabled, keep: data.keep } : null)

  const clear = () => {
    commands.clearSavedAudio()
      .then(() => toast.success('Saved audio deleted'))
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
            {shown?.enabled ? `Last ${shown.keep} entries${data ? ` · ${fmtBytes(data.bytes)}` : ''}` : 'Off'}
          </p>
        }
      >
        <span
          className={cn('inline-flex shrink-0 items-center rounded-lg bg-field ring-1 ring-line-soft transition-opacity ring-inset', !shown?.enabled && 'pointer-events-none opacity-40')}
          data-tip="How many recent entries keep their audio"
          data-tip-align="end"
        >
          <button type="button" aria-label="Fewer" disabled={!shown || shown.keep <= MIN_KEEP} onClick={() => shown && void choose({ ...shown, keep: shown.keep - 1 })} className={stepper}>
            <MinusIcon size={11} weight="bold" />
          </button>
          <b className="w-7 text-center text-sm font-semibold text-fg tabular-nums">{shown?.keep ?? '–'}</b>
          <button type="button" aria-label="More" disabled={!shown || shown.keep >= MAX_KEEP} onClick={() => shown && void choose({ ...shown, keep: shown.keep + 1 })} className={stepper}>
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
          className={cn('grid size-7 shrink-0 cursor-pointer place-items-center rounded-lg text-muted hover:bg-danger-soft hover:text-danger disabled:pointer-events-none disabled:opacity-40', !shown?.enabled && 'invisible')}
        >
          <TrashIcon size={14} />
        </button>
        <Switch
          aria-label="Saved audio"
          checked={shown?.enabled ?? false}
          disabled={!shown}
          onChange={(enabled) => shown && void choose({ ...shown, enabled })}
        />
      </SettingRow>
    </SettingsCard>
  )
}
