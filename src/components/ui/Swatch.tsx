import type { ReactNode } from 'react'
import { CheckIcon } from '@phosphor-icons/react'
import { cn } from '@/lib/utils'

type Props = {
  active: boolean
  onPick: () => void
  /** What the choice looks like: a colour strip, a mini pill. */
  preview: ReactNode
  label: string
  hint: string
  /** The hint beside the label instead of under it. */
  inline?: boolean
}

/** One option in an Appearance picker. */
export function Swatch({ active, onPick, preview, label, hint, inline = false }: Props) {
  return (
    <button
      type="button"
      onClick={onPick}
      aria-pressed={active}
      className={cn(
        'flex h-10 cursor-pointer items-center gap-2.5 rounded-md border px-2.5 text-left transition-[background-color,border-color,scale] duration-150 active:scale-99',
        active ? 'border-accent bg-accent-soft' : 'border-line-soft bg-surface hover:border-line hover:bg-surface-hover',
      )}
    >
      {preview}
      <span className={cn('min-w-0 flex-1', inline && 'truncate')}>
        <span className={cn(
          'text-[12px] font-semibold tracking-[-0.01em] transition-colors duration-200',
          active ? 'text-accent' : 'text-fg',
          !inline && 'block truncate',
        )}>
          {label}
        </span>
        <span className={cn('text-[10px] text-muted', inline ? 'ml-2' : 'block truncate')}>{hint}</span>
      </span>
      {active && (
        <span className="grid size-4 shrink-0 place-items-center rounded-full bg-accent">
          <CheckIcon size={9} weight="bold" className="text-accent-fg" />
        </span>
      )}
    </button>
  )
}
