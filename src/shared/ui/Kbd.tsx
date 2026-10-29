import type { ReactNode } from 'react'
import { cn } from '@/shared/utils'

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
