import type { CSSProperties, ReactNode } from 'react'
import { motion } from 'framer-motion'
import { Dialog } from 'radix-ui'
import { X } from 'lucide-react'
import { cn } from '@/lib/utils'
import { IconButton } from './button'

type Props = {
  /** Omit to make the dialog blocking. */
  onClose?: () => void
  title: ReactNode
  description?: ReactNode
  icon?: ReactNode
  width?: number
  children: ReactNode
}

const PANEL_EASE = [0.22, 1, 0.36, 1] as const

/** Mount while open, inside `AnimatePresence` for the exit motion. */
export function Modal({ onClose, title, description, icon, width = 460, children }: Props) {
  const blocking = !onClose
  const block = (e: Event) => e.preventDefault()

  return (
    <Dialog.Root open onOpenChange={(next) => { if (!next) onClose?.() }}>
      <Dialog.Portal forceMount>
        <Dialog.Overlay asChild>
          <motion.div
            className="nv-overlay"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.18 }}
          />
        </Dialog.Overlay>
        <div className="nv-modal-wrap">
          <Dialog.Content
            asChild
            {...(description ? {} : { 'aria-describedby': undefined })}
            onOpenAutoFocus={block}
            onEscapeKeyDown={blocking ? block : undefined}
            onInteractOutside={blocking ? block : undefined}
          >
            <motion.div
              className="nv-modal"
              style={{ '--modal-w': `${width}px` } as CSSProperties}
              initial={{ opacity: 0, scale: 0.96, y: 10 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.97, y: 6 }}
              transition={{ duration: 0.22, ease: PANEL_EASE }}
            >
              <div className="nv-modal__head">
                {icon}
                <div className="min-w-0 flex-1">
                  <Dialog.Title className="nv-modal__title">{title}</Dialog.Title>
                  {description && <Dialog.Description className="nv-modal__desc">{description}</Dialog.Description>}
                </div>
                {!blocking && (
                  <Dialog.Close asChild>
                    <IconButton label="Close" tone="danger" size="sm" className="-mr-1.5 -mt-1">
                      <X strokeWidth={2} />
                    </IconButton>
                  </Dialog.Close>
                )}
              </div>
              {children}
            </motion.div>
          </Dialog.Content>
        </div>
      </Dialog.Portal>
    </Dialog.Root>
  )
}

export function ModalBody({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={cn('nv-modal__body', className)}>{children}</div>
}

export function ModalFoot({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={cn('nv-modal__foot', className)}>{children}</div>
}
