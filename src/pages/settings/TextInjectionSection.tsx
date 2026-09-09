import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Check, RefreshCw, ClipboardType } from 'lucide-react'
import { COMMANDS } from '../../lib/commands'
import { Section } from '../../components/Section'
import type { InjectionStatus } from '../../types'

/**
 * Linux needs an external helper to type transcripts, because Wayland does not
 * let an application send keystrokes to another window. Which helper works
 * depends on the compositor, so the choice is shown rather than left to fail
 * silently — a missing tool means nothing is ever pasted.
 *
 * Renders nothing on Windows and macOS, where the clipboard needs no setup.
 */
export function TextInjectionSection() {
  const [status, setStatus] = useState<InjectionStatus | null>(null)
  const [checking, setChecking] = useState(false)

  const load = useCallback(
    () =>
      invoke<InjectionStatus>(COMMANDS.GET_INJECTION_STATUS)
        .then(setStatus)
        .catch(() => setStatus(null)),
    [],
  )

  // The probe spawns a process per tool, so re-read on demand rather than on
  // an interval; the set only changes when the user installs something.
  const recheck = useCallback(() => {
    setChecking(true)
    void load().finally(() => setChecking(false))
  }, [load])

  useEffect(() => { void load() }, [load])

  if (!status?.configurable) return null

  const ready = status.selected !== null

  return (
    <Section
      title="Text injection"
      Icon={ClipboardType}
      description={
        ready
          ? `Typing transcripts with ${status.selected} on ${status.session}.`
          : `No supported tool found on ${status.session}. Install one below, or nothing will be typed.`
      }
      action={
        <button
          type="button"
          onClick={recheck}
          disabled={checking}
          className="btn btn-sm btn-quiet"
        >
          <RefreshCw size={11} strokeWidth={1.9} className={checking ? 'animate-spin' : undefined} />
          Check again
        </button>
      }
      bodyClassName="flex flex-col gap-1.5 p-3"
    >
      {status.tools.map((tool) => (
        <div
          key={tool.name}
          className={`flex items-center gap-2.5 rounded-(--r-md) px-2.5 py-1.5 ${
            tool.preferred ? 'bg-(--accent-soft)' : 'bg-(--surface)'
          }`}
        >
          <span className={`shrink-0 ${tool.available ? 'text-(--on-soft)' : 'text-(--faint)'}`}>
            {tool.available ? <Check size={12} strokeWidth={2.5} /> : <span className="block size-3" />}
          </span>

          <code className={`shrink-0 text-[11.5px] font-semibold ${tool.preferred ? 'text-(--on-soft)' : 'text-(--fg-2)'}`}>
            {tool.name}
          </code>

          {tool.preferred && (
            <span className="shrink-0 text-[10.5px] font-medium text-(--on-soft)">in use</span>
          )}

          {!tool.available && (
            <span className="min-w-0 flex-1 truncate text-right text-[10.5px] text-(--muted)" title={tool.installHint}>
              {tool.installHint}
            </span>
          )}
        </div>
      ))}
    </Section>
  )
}
