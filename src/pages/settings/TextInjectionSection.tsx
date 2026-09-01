import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Check, KeyboardIcon, RefreshCw, TriangleAlert } from 'lucide-react'
import { COMMANDS } from '../../lib/commands'
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
    <div className="rounded-(--r-lg) border border-(--border-soft) bg-(--panel) p-4">
      <div className="flex items-center gap-2.5">
        <span
          className={`grid size-7 shrink-0 place-items-center rounded-(--r-sm) ${
            ready ? 'bg-(--accent-soft) text-(--accent)' : 'text-destructive'
          }`}
        >
          {ready ? <KeyboardIcon size={14} strokeWidth={2} /> : <TriangleAlert size={14} strokeWidth={2} />}
        </span>

        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-bold tracking-[-0.02em] text-(--fg)">Text injection</p>
          <p className="mt-0.5 text-[11px] text-muted-foreground">
            {ready
              ? `Typing transcripts with ${status.selected} on ${status.session}.`
              : `No supported tool found on ${status.session}. Install one below, or nothing will be typed.`}
          </p>
        </div>

        <button
          type="button"
          onClick={recheck}
          disabled={checking}
          title="Check again"
          className="flex shrink-0 items-center gap-1.5 rounded-(--r-sm) border border-(--border) bg-(--surface) px-2 py-1 text-[11px] font-medium text-(--fg-2) cursor-pointer transition-colors duration-(--t-fast) hover:bg-accent hover:text-(--fg) disabled:opacity-50"
        >
          <RefreshCw size={11} strokeWidth={1.75} className={checking ? 'animate-spin' : undefined} />
          <span className="leading-none">Recheck</span>
        </button>
      </div>

      <ul className="mt-3 flex flex-col gap-1.5">
        {status.tools.map((tool) => (
          <li
            key={tool.name}
            className={`flex items-center gap-2.5 rounded-(--r-md) border px-2.5 py-1.5 ${
              tool.preferred
                ? 'border-(--accent-soft) bg-(--accent-soft)'
                : 'border-(--border-soft) bg-(--surface)'
            }`}
          >
            <span className={`shrink-0 ${tool.available ? 'text-(--accent)' : 'text-muted-foreground'}`}>
              {tool.available ? <Check size={12} strokeWidth={2.5} /> : <span className="block size-3" />}
            </span>

            <code className={`shrink-0 text-[11.5px] font-semibold ${tool.preferred ? 'text-(--accent)' : 'text-(--fg-2)'}`}>
              {tool.name}
            </code>

            {tool.preferred && (
              <span className="shrink-0 rounded-(--r-xs) bg-(--accent-soft) px-1.5 py-px text-[9px] font-semibold uppercase tracking-[0.04em] text-(--accent)">
                in use
              </span>
            )}

            {!tool.available && (
              <span className="min-w-0 flex-1 truncate text-right text-[10.5px] text-muted-foreground" title={tool.installHint}>
                {tool.installHint}
              </span>
            )}
          </li>
        ))}
      </ul>
    </div>
  )
}
