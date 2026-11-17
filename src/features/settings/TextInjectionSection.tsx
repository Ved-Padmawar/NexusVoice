import { useCallback, useEffect, useRef, useState } from 'react'
import { CheckIcon, KeyboardIcon, ArrowsClockwiseIcon, WarningIcon, MinusIcon, InfoIcon } from '@phosphor-icons/react'
import { commands } from '../../shared/bindings'
import { Button } from '../../shared/ui/Button'
import { CopyButton } from '../../shared/ui/CopyButton'
import { Popover } from '../../shared/ui/Popover'
import type { InjectionStatus, InjectionTool } from '../../shared/types'

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
      commands.getInjectionStatus()
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
    <section>
      <h2 className="mb-2 flex items-center gap-2 text-sm font-semibold text-fg-2">
        Text injection
        <span className="rounded-md bg-accent-2-soft px-1.5 py-px text-[10px] font-semibold text-accent-2-ink">Linux</span>
      </h2>
      <div className="rounded-2xl bg-panel p-4 ring-1 ring-line-soft ring-inset">
        <div className="flex items-center gap-3">
          <span
            className={`grid size-8 shrink-0 place-items-center rounded-[10px] ${
              ready ? 'bg-success-soft text-success' : 'bg-danger-soft text-danger'
            }`}
          >
            {ready ? <KeyboardIcon size={16} weight="duotone" /> : <WarningIcon size={16} weight="duotone" />}
          </span>

          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-semibold text-fg">
              {ready ? `Typing with ${status.selected} on ${status.session}` : `No supported tool found on ${status.session}`}
            </p>
            <p className="truncate text-xs text-muted">
              {ready
                ? 'Wayland blocks pasting into other windows, so a helper types for NexusVoice.'
                : 'Install one below, or nothing will be typed.'}
            </p>
          </div>

          <Button onClick={recheck} disabled={checking}>
            <ArrowsClockwiseIcon size={12} weight="bold" className={checking ? 'animate-spin' : undefined} />
            Check again
          </Button>
        </div>

        <ul className="mt-3 grid grid-cols-4 gap-1.5">
          {status.tools.map((tool) => (
            <li
              key={tool.name}
              className={`flex h-8 items-center gap-1.5 rounded-lg pr-0.5 pl-2.5 ring-1 ring-inset ${
                tool.preferred ? 'bg-accent-soft ring-accent-line' : 'bg-field ring-line-soft'
              }`}
            >
              {tool.available
                ? <CheckIcon size={12} weight="bold" className={tool.preferred ? 'text-accent-ink' : 'text-success'} />
                : <MinusIcon size={12} className="text-muted" />}
              <code className={`min-w-0 flex-1 truncate text-xs font-semibold ${tool.preferred ? 'text-accent-ink' : 'text-fg-2'}`}>{tool.name}</code>
              {!tool.available && <InstallInfo tool={tool} />}
            </li>
          ))}
        </ul>
      </div>
    </section>
  )
}

/** The ⓘ on a missing tool. Opens on click, not hover, so the box stays put
 *  while a command is copied. */
function InstallInfo({ tool }: { tool: InjectionTool }) {
  const [open, setOpen] = useState(false)
  const anchorRef = useRef<HTMLButtonElement>(null)

  return (
    <>
      <button
        ref={anchorRef}
        type="button"
        aria-label={`How to install ${tool.name}`}
        aria-expanded={open}
        onClick={() => setOpen(o => !o)}
        className={`grid size-7 shrink-0 cursor-pointer place-items-center rounded-md transition-colors hover:text-fg ${open ? 'text-fg' : 'text-muted'}`}
      >
        <InfoIcon size={14} />
      </button>
      <Popover open={open} onOpenChange={setOpen} anchorRef={anchorRef} placement="bottom-end" aria-label={`Install ${tool.name}`} className="w-72">
        {tool.install.length > 0 && (
          <ul className="divide-y divide-line-soft">
            {tool.install.map(command => (
              <li key={command} className="flex items-center gap-2 py-1 pr-1.5 pl-3.5">
                <code className="min-w-0 flex-1 truncate text-xs text-fg-2 select-all">{command}</code>
                <CopyButton text={command} label={`Copy ${command}`} />
              </li>
            ))}
          </ul>
        )}
        {tool.note && (
          <p className={`px-3.5 py-2.5 text-xs text-muted ${tool.install.length > 0 ? 'border-t border-line-soft' : ''}`}>{tool.note}</p>
        )}
      </Popover>
    </>
  )
}
