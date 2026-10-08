import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Timer } from 'lucide-react'
import { toast } from 'sonner'
import { COMMANDS } from '../../lib/commands'
import { extractErrorMessage } from '../../lib/errors'
import type { ModelUnload } from '../../types'

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
  // `null` until the saved value arrives; the buttons wait for it, so a late
  // read can never overwrite a click.
  const [policy, setPolicy] = useState<ModelUnload | null>(null)

  useEffect(() => {
    invoke<ModelUnload>(COMMANDS.GET_MODEL_UNLOAD).then(setPolicy).catch(() => setPolicy('never'))
  }, [])

  const choose = async (next: ModelUnload) => {
    if (policy === null || next === policy) return
    const previous = policy
    setPolicy(next)
    try {
      await invoke(COMMANDS.SET_MODEL_UNLOAD, { policy: next })
    } catch (e) {
      setPolicy(previous)
      toast.error(extractErrorMessage(e, 'Could not save the unload setting'))
    }
  }

  return (
    <div className="flex shrink-0 items-center gap-2.5 rounded-(--r-md) border border-(--border-soft) bg-(--panel) px-3 py-2">
      <Timer size={13} strokeWidth={2} className="shrink-0 text-(--accent)" />
      <span className="shrink-0 text-[12px] font-semibold text-(--fg-2)">Unload when idle</span>
      <span className="min-w-0 flex-1 truncate text-[10px] text-muted-foreground">
        Frees memory between dictations; the next one waits for the model to reload.
      </span>
      <div role="radiogroup" aria-label="Unload when idle" className="flex shrink-0 items-center gap-1">
        {OPTIONS.map((option) => {
          const on = option.value === policy
          return (
            <button
              key={option.value}
              type="button"
              role="radio"
              aria-checked={on}
              disabled={policy === null}
              onClick={() => void choose(option.value)}
              className={`nv-edge rounded-full px-2.5 py-1 text-[11px] font-medium cursor-pointer disabled:cursor-default disabled:opacity-50 ${
                on
                  ? '[--edge:color-mix(in_srgb,var(--accent)_55%,transparent)] bg-(--accent-soft) text-(--accent)'
                  : 'text-(--fg-2) hover:[--edge:var(--muted)] hover:text-(--fg)'
              }`}
            >
              {option.label}
            </button>
          )
        })}
      </div>
    </div>
  )
}
