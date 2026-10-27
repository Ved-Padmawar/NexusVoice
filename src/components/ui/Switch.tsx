import { cn } from '@/lib/utils'

type Props = {
  checked: boolean
  onChange: (checked: boolean) => void
  disabled?: boolean
  className?: string
  'aria-label': string
}

/** A styled checkbox: the knob stretches while pressed and slides on release. */
export function Switch({ checked, onChange, disabled, className, 'aria-label': label }: Props) {
  return (
    <input
      type="checkbox"
      role="switch"
      aria-label={label}
      checked={checked}
      disabled={disabled}
      onChange={(e) => onChange(e.target.checked)}
      className={cn(
        'relative h-6 w-10 shrink-0 cursor-pointer appearance-none rounded-full bg-[color-mix(in_oklab,var(--fg)_18%,var(--surface))] transition-colors checked:bg-accent disabled:cursor-not-allowed disabled:opacity-50',
        "before:absolute before:top-0.75 before:left-0.75 before:size-4.5 before:rounded-full before:bg-white before:shadow before:ring-1 before:ring-black/40before:transition-[translate,width] before:duration-220 before:ease-out-quint before:content-['']",
        'checked:before:translate-x-4 active:before:w-5.5 checked:active:before:translate-x-3',
        className,
      )}
    />
  )
}
