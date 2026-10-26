import type { ComponentProps, ReactNode } from 'react'
import { cn } from '@/lib/utils'

type Props = ComponentProps<'input'> & {
  /** Leading icon, drawn inside the field. */
  icon?: ReactNode
  /** Drawn inside the field's right end, e.g. a shortcut hint. */
  trailing?: ReactNode
  /** Classes for the wrapper, which sizes the field. */
  wrapperClassName?: string
}

export function TextInput({ icon, trailing, wrapperClassName, className, ...props }: Props) {
  return (
    <span className={cn('relative flex items-center', wrapperClassName)}>
      {icon && <span className="pointer-events-none absolute left-3 flex text-sm text-muted">{icon}</span>}
      <input
        className={cn(
          'h-9 w-full rounded-lg bg-field px-3 text-sm text-fg ring-1 ring-line-soft ring-inset outline-none transition-shadow placeholder:text-muted hover:ring-line focus:ring-accent disabled:cursor-not-allowed disabled:opacity-50',
          icon && 'pl-9',
          trailing && 'pr-9',
          className,
        )}
        {...props}
      />
      {trailing && <span className="pointer-events-none absolute right-2.5 flex">{trailing}</span>}
    </span>
  )
}
