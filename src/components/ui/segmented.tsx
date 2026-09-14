import { useId, type ReactNode } from 'react'
import { motion } from 'framer-motion'
import { cn } from '@/lib/utils'

type Option<T extends string> = {
  value: T
  label: ReactNode
  count?: number
}

type Props<T extends string> = {
  value: T
  onChange: (value: T) => void
  options: Option<T>[]
  /** Accessible name of the group. */
  label: string
  block?: boolean
  className?: string
}

/** Mutually exclusive options; the selection slides between them. */
export function Segmented<T extends string>({ value, onChange, options, label, block, className }: Props<T>) {
  const layoutId = useId()
  return (
    <div role="group" aria-label={label} className={cn('nv-seg', block && 'nv-seg--block', className)}>
      {options.map((opt) => {
        const on = opt.value === value
        return (
          <button
            key={opt.value}
            type="button"
            aria-pressed={on}
            onClick={() => onChange(opt.value)}
            className="nv-seg__item"
          >
            {on && (
              <motion.span
                layoutId={layoutId}
                className="nv-seg__ind"
                transition={{ type: 'spring', stiffness: 520, damping: 40 }}
              />
            )}
            {opt.label}
            {opt.count !== undefined && <span className="nv-seg__count">{opt.count}</span>}
          </button>
        )
      })}
    </div>
  )
}
