import { TimerIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { Select } from '../../shared/ui/Select'
import { SettingRow } from './SettingRow'
import { useSetting } from './queries'
import type { ModelUnload } from '../../shared/types'

const OPTIONS: { value: ModelUnload; label: string }[] = [
  { value: 'never', label: 'Never' },
  { value: 'after2Minutes', label: '2 min' },
  { value: 'after5Minutes', label: '5 min' },
  { value: 'after10Minutes', label: '10 min' },
  { value: 'after15Minutes', label: '15 min' },
  { value: 'after1Hour', label: '1 hour' },
]

const saveUnload = (policy: ModelUnload) => commands.setModelUnload(policy)
const saveFailed = (e: unknown) => toast.error(extractErrorMessage(e, 'Could not save the unload setting'))

/** When an idle model is unloaded to free memory. */
export function ModelUnloadSection() {
  const [policy, choose] = useSetting('modelUnload', saveUnload, saveFailed)

  return (
    <SettingRow
      icon={<TimerIcon size={16} weight="duotone" />}
      title="Unload the model when idle"
      description={<p className="text-xs text-pretty text-muted">Frees memory; the next recording reloads it.</p>}
    >
      <div className="w-28 shrink-0">
        <Select
          value={policy}
          options={OPTIONS}
          onChange={(v) => { if (v !== policy) choose(v) }}
          aria-label="Unload idle model"
        />
      </div>
    </SettingRow>
  )
}
