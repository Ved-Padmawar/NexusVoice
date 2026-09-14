import type { ReactNode } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { Popover } from 'radix-ui'
import { cn } from '@/lib/utils'

type Props = {
  open: boolean
  children: ReactNode
  align?: 'start' | 'center' | 'end'
  sideOffset?: number
  className?: string
}

/** Animated content for a Radix `Popover.Root`; owns the portal. */
export function PopoverPanel({ open, children, align = 'end', sideOffset = 6, className }: Props) {
  return (
    <AnimatePresence>
      {open && (
        <Popover.Portal forceMount>
          <Popover.Content align={align} sideOffset={sideOffset} collisionPadding={12} asChild>
            <motion.div
              initial={{ opacity: 0, y: -4, scale: 0.97 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -4, scale: 0.97 }}
              transition={{ duration: 0.15, ease: [0.22, 1, 0.36, 1] }}
              style={{ transformOrigin: 'var(--radix-popover-content-transform-origin)' }}
              className={cn('nv-pop', className)}
            >
              {children}
            </motion.div>
          </Popover.Content>
        </Popover.Portal>
      )}
    </AnimatePresence>
  )
}
