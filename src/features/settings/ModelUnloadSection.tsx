import { useEffect, useState } from 'react'
import { TimerIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { Select } from '../../shared/ui/Select'
import type { ModelUnload } from '../../shared/types'

const OPTIONS: { value: ModelUnload; label: string }[] = [
  { value: 'never', label: 'Never' },
  { value: 'after2Minutes', label: '2 min' },
  { value: 'after5Minutes', label: '5 min' },
  { value: 'after10Minutes', label: '10 min' },
  { value: 'after15Minutes', label: '15 min' },
  { value: 'after1Hour', label: '1 hour' },
]

/** When an idle model is unloaded to free memory. */
export function ModelUnloadSection() {
  // `null` until the saved value arrives; the select waits for it, so a late
  // read can never overwrite a click.
  const [policy, setPolicy] = useState<ModelUnload | null>(null)

  useEffect(() => {
    commands.getModelUnload().then(setPolicy).catch(() => setPolicy('never'))
  }, [])

  const choose = async (next: ModelUnload) => {
    if (policy === null || next === policy) return
    const previous = policy
    setPolicy(next)
    try {
      await commands.setModelUnload(next)
    } catch (e) {
      setPolicy(previous)
      toast.error(extractErrorMessage(e, 'Could not save the unload setting'))
    }
  }

  return (
    <div className="inline-flex h-8 items-center gap-2 rounded-lg bg-panel pr-1 pl-3 text-xs ring-1 ring-line-soft ring-inset">
      <TimerIcon size={14} weight="bold" className="text-accent-ink" />
      <span className="font-semibold text-fg">Unload when idle</span>
      <div className="w-22">
        <Select
          size="sm"
          value={policy}
          options={OPTIONS}
          onChange={(v) => void choose(v)}
          aria-label="Unload when idle"
          disabled={policy === null}
        />
      </div>
    </div>
  )
}
