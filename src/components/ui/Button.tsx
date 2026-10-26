import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

const VARIANTS = {
  primary: 'bg-accent text-accent-fg hover:bg-accent-hover',
  tinted: 'bg-accent-soft text-accent-ink ring-1 ring-inset ring-accent-line hover:bg-accent/25 hover:ring-accent',
  outline: 'text-fg ring-1 ring-inset ring-line hover:bg-surface-hover',
  chip: 'bg-panel text-fg ring-1 ring-inset ring-line-soft hover:ring-line',
  ghost: 'text-fg-2 hover:bg-surface-hover hover:text-fg',
  danger: 'text-danger ring-1 ring-inset ring-danger/30 hover:bg-danger-soft hover:ring-danger/60',
} as const

const SIZES = {
  sm: 'h-7.5 px-3 text-xs',
  md: 'h-8 px-3.5 text-xs',
  lg: 'h-9 px-5 text-sm',
  icon: 'size-8',
  'icon-sm': 'size-7',
} as const

export type ButtonVariant = keyof typeof VARIANTS
export type ButtonSize = keyof typeof SIZES

type Props = ComponentProps<'button'> & { variant?: ButtonVariant; size?: ButtonSize }

export function Button({ variant = 'outline', size = 'md', type = 'button', className, ...props }: Props) {
  return (
    <button
      type={type}
      className={cn(
        'inline-flex shrink-0 cursor-pointer items-center justify-center gap-1.5 rounded-lg font-semibold whitespace-nowrap transition-[color,background-color,box-shadow] duration-120 disabled:pointer-events-none disabled:opacity-50',
        VARIANTS[variant],
        SIZES[size],
        className,
      )}
      {...props}
    />
  )
}
