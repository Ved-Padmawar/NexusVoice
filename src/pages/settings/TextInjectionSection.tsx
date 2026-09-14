import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Check, RefreshCw, TriangleAlert } from 'lucide-react'
import { COMMANDS } from '../../lib/commands'
import { Button } from '@/components/ui/button'
import { Section } from '../../components/page'
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
      title={<>Text injection {!ready && <TriangleAlert size={15} strokeWidth={2} className="text-warning" aria-label="Needs attention" />}</>}
      description={ready
        ? `Typing transcripts with ${status.selected} on ${status.session}.`
        : `No supported tool found on ${status.session}. Install one below, or nothing will be typed.`}
      actions={
        <Button variant="secondary" size="sm" onClick={recheck} disabled={checking}>
          <RefreshCw className={checking ? 'nv-spin' : undefined} />
          Check again
        </Button>
      }
    >
      <div className="nv-card nv-group">
        {status.tools.map((tool) => (
          <div key={tool.name} className="nv-row nv-row--inline py-3!">
            <div className="flex min-w-0 items-center gap-3">
              <span className={`grid size-5 shrink-0 place-items-center ${tool.available ? 'text-success' : 'text-faint'}`}>
                {tool.available
                  ? <Check size={15} strokeWidth={2.5} aria-label="Installed" />
                  : <span className="size-1.5 rounded-full bg-faint" aria-label="Not installed" />}
              </span>
              <span className={`text-[13px] font-semibold ${tool.preferred ? 'text-fg' : 'text-fg-2'}`}>{tool.name}</span>
              {tool.preferred && <span className="nv-badge">In use</span>}
            </div>
            {!tool.available && (
              <span className="min-w-0 truncate text-right text-[12px] text-muted" title={tool.installHint}>
                {tool.installHint}
              </span>
            )}
          </div>
        ))}
      </div>
    </Section>
  )
}
