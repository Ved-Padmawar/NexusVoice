import { useEffect } from 'react'
import { ClipboardTextIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { queryKeys } from '../../shared/queryKeys'
import { isWindows } from '../../shared/platform'
import { useLiveChoice } from '../../app/useLiveChoice'
import { Select } from '../../shared/ui/Select'
import { SettingRow } from './SettingRow'
import { usePasteMode } from './queries'
import type { PasteMode } from '../../shared/types'

const MODES: { value: PasteMode; label: string; about: string }[] = [
  { value: 'auto', label: 'Auto', about: 'Ctrl+Shift+V in terminals, Ctrl+V everywhere else.' },
  { value: 'ctrlV', label: 'Ctrl+V', about: 'The usual paste. Most apps.' },
  { value: 'ctrlShiftV', label: 'Ctrl+Shift+V', about: 'Terminals, and pasting without formatting.' },
  { value: 'shiftInsert', label: 'Shift+Insert', about: 'Older apps and some terminals.' },
  { value: 'copyOnly', label: 'Copy only', about: 'Puts the transcript on the clipboard. You paste it.' },
]

const TIP = MODES.map(m => `${m.label}: ${m.about}`).join('\n')

const savePasteMode = (mode: PasteMode) => commands.setPasteMode(mode)
const saveFailed = (e: unknown) => toast.error(extractErrorMessage(e, 'Could not save the paste setting'))

/** How the transcript gets into the focused app. Only the Windows clipboard
 *  route has a choice: macOS always sends Cmd+V and Linux types the text. */
export function PasteModeSection() {
  const { data, error } = usePasteMode(isWindows)
  const { pending, choose } = useLiveChoice(queryKeys.pasteMode, savePasteMode, saveFailed)

  useEffect(() => {
    if (error) toast.error(extractErrorMessage(error, 'Could not load the paste setting'))
  }, [error])

  if (!isWindows) return null

  const mode = pending ? pending.value : data ?? null

  return (
    <SettingRow
      icon={<ClipboardTextIcon size={16} weight="duotone" />}
      title="Paste with"
      tip={TIP}
      description={<p className="text-xs text-pretty text-muted">How the transcript gets into your app.</p>}
    >
      <div className="w-40 shrink-0">
        <Select
          value={mode}
          options={MODES}
          onChange={(v) => { if (v !== mode) void choose(v) }}
          aria-label="Paste with"
          disabled={mode === null}
        />
      </div>
    </SettingRow>
  )
}
