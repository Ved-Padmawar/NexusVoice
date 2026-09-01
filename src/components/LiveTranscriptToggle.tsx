import { emit } from '@tauri-apps/api/event'
import { motion } from 'framer-motion'
import { Captions } from 'lucide-react'
import { EVENTS } from '../lib/events'
import { useAppStore } from '../store/useAppStore'

/** Expand the pill into a live transcript card as you speak. */
export function LiveTranscriptToggle() {
  const liveTranscript = useAppStore((s) => s.liveTranscript)
  const setLiveTranscript = useAppStore((s) => s.setLiveTranscript)

  const toggle = () => {
    const next = !liveTranscript
    setLiveTranscript(next)
    void emit(EVENTS.PILL_LIVE_TRANSCRIPT_CHANGED, next)
  }

  return (
    <div className="nv-edge [--edge:var(--border-soft)] flex items-center justify-between gap-4 rounded-(--r-lg) bg-(--panel) px-4 py-3">
      <div className="flex min-w-0 items-center gap-3">
        <div className="flex size-8 shrink-0 items-center justify-center rounded-(--r-md) bg-(--accent-soft) text-(--accent)">
          <Captions size={14} strokeWidth={2} />
        </div>
        <div className="min-w-0">
          <p className="text-[12px] font-semibold tracking-[-0.01em] text-(--fg-2)">Live transcript in the pill</p>
          <p className="mt-0.75 text-[11px] text-muted-foreground">
            The pill expands into a card and fills in as you speak. A large
            model on modest hardware may lag behind your voice.
          </p>
        </div>
      </div>

      <motion.button
        type="button"
        role="switch"
        aria-checked={liveTranscript}
        aria-label="Live transcript in the pill"
        onClick={toggle}
        className="relative h-6 w-10.5 shrink-0 cursor-pointer rounded-full border-none p-0"
        initial={false}
        animate={{ backgroundColor: liveTranscript ? 'var(--accent)' : 'var(--border)' }}
        whileTap={{ scale: 0.95 }}
        transition={{ duration: 0.2 }}
      >
        <motion.span
          className="absolute top-0.75 left-0.75 size-4.5 rounded-full bg-white shadow-sm"
          initial={false}
          animate={{ x: liveTranscript ? 18 : 0 }}
          transition={{ type: 'spring', stiffness: 400, damping: 28 }}
        />
      </motion.button>
    </div>
  )
}
