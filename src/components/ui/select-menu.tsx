import type { CSSProperties, ReactNode } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { Select } from 'radix-ui'
import { Check, ChevronDown } from 'lucide-react'
import { cn } from '@/lib/utils'

type Props = {
  value: string
  onValueChange: (value: string) => void
  open: boolean
  onOpenChange: (open: boolean) => void
  disabled?: boolean
  icon: ReactNode
  /** Trigger's accessible name. */
  label: string
  display: ReactNode
  /** Pinned above the list. */
  header?: ReactNode
  panelStyle?: CSSProperties
  children: ReactNode
}

export function SelectMenu({
  value, onValueChange, open, onOpenChange, disabled, icon, label, display, header, panelStyle, children,
}: Props) {
  return (
    <Select.Root value={value} onValueChange={onValueChange} open={open} onOpenChange={onOpenChange}>
      <Select.Trigger asChild disabled={disabled}>
        <button type="button" aria-label={label} className="nv-trigger">
          {icon}
          <span className="min-w-0 flex-1 truncate">
            <Select.Value>{display}</Select.Value>
          </span>
          <motion.span
            className="nv-trigger__chevron"
            animate={{ rotate: open ? 180 : 0 }}
            transition={{ duration: 0.18 }}
          >
            <ChevronDown size={15} strokeWidth={2} />
          </motion.span>
        </button>
      </Select.Trigger>

      <AnimatePresence>
        {open && (
          <Select.Portal forceMount>
            <Select.Content asChild position="popper" sideOffset={6} collisionPadding={12}>
              <motion.div
                initial={{ opacity: 0, y: -4, scale: 0.98 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: -4, scale: 0.98 }}
                transition={{ duration: 0.15, ease: [0.22, 1, 0.36, 1] }}
                style={{ maxHeight: '18rem', transformOrigin: 'var(--radix-select-content-transform-origin)', ...panelStyle }}
                className="nv-pop flex w-(--radix-select-trigger-width) flex-col"
              >
                {header}
                <Select.Viewport
                  className="nv-menu nv-scroll-quiet min-h-0 flex-1"
                  // Radix inlines `overflow: hidden auto` here, beating the class.
                  style={{ overflowY: 'auto', overscrollBehavior: 'none' }}
                >
                  {children}
                </Select.Viewport>
              </motion.div>
            </Select.Content>
          </Select.Portal>
        )}
      </AnimatePresence>
    </Select.Root>
  )
}

export function SelectMenuItem({ value, children, className }: { value: string; children: ReactNode; className?: string }) {
  return (
    <Select.Item value={value} className={cn('nv-menu-item', className)}>
      <span className="min-w-0 flex-1 truncate">
        <Select.ItemText>{children}</Select.ItemText>
      </span>
      <Select.ItemIndicator className="flex shrink-0">
        <Check size={14} strokeWidth={2.5} />
      </Select.ItemIndicator>
    </Select.Item>
  )
}
