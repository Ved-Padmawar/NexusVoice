import type { ReactNode } from 'react'
import { cn } from '../utils'
import { SUPER_KEY_LABEL } from '../platform'

export function Kbd({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <kbd
      className={cn(
        'inline-grid h-5.25 min-w-5.5 place-items-center rounded-sm bg-[light-dark(var(--surface-hover),var(--panel))] px-1.5 pb-0.5 font-mono text-2xs leading-none font-semibold text-fg tabular-nums slashed-zero shadow-[inset_0_0_0_1px_var(--border),inset_0_-2px_0_var(--border)]',
        className,
      )}
    >
      {children}
    </kbd>
  )
}

const KEY_LABEL: Record<string, string> = {
  // Super is the Windows logo key on Win/Linux and Command on macOS — the
  // accelerator token stays "Super" everywhere; only the label differs.
  Super: SUPER_KEY_LABEL, Return: 'Enter', Delete: 'Del',
  ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right',
}

/** A shortcut's keys, by their accelerator tokens (`['Ctrl', 'Super']`), as labelled keycaps. */
export function HotkeyKeys({ keys }: { keys: readonly string[] }) {
  return (
    <span className="inline-flex items-center gap-1 align-middle">
      {keys.map((k, i) => <Kbd key={`${k}-${i}`}>{KEY_LABEL[k] ?? k}</Kbd>)}
    </span>
  )
}
