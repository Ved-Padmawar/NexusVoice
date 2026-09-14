import { motion } from 'framer-motion'
import { cn } from '@/lib/utils'
import { displayKey } from '../../lib/hotkeys'

type Props = {
  /** Accelerator tokens, e.g. `['Ctrl', 'Space']`. */
  keys: string[]
  /** Keys sink on hover. */
  pressable?: boolean
  /** Keys spring in as they are recorded. */
  live?: boolean
  className?: string
}

/** Sized in em, so it scales with the surrounding text. */
export function KeyCombo({ keys, pressable, live, className }: Props) {
  return (
    <span className={cn('nv-keys', pressable && 'nv-keys--pressable', className)}>
      {keys.map((k, i) => (
        <span key={`${k}-${i}`} className="contents">
          {i > 0 && <span className="nv-keys__plus" aria-hidden>+</span>}
          {live ? (
            <motion.kbd
              className="nv-key"
              initial={{ scale: 0.6, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              transition={{ type: 'spring', stiffness: 520, damping: 26 }}
            >
              {displayKey(k)}
            </motion.kbd>
          ) : (
            <kbd className="nv-key">{displayKey(k)}</kbd>
          )}
        </span>
      ))}
    </span>
  )
}
