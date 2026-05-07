import { memo } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { emit } from '@tauri-apps/api/event'
import { Check } from 'lucide-react'
import { useAppStore } from '../../store/useAppStore'
import { EVENTS } from '../../lib/events'
import type { PillTheme } from '../../store/uiSlice'

type PillThemeDef = {
  id: PillTheme
  label: string
  bg: string
  border: string
  accent: string
  brand: string
}

const PILL_THEMES: PillThemeDef[] = [
  { id: 'dark',  label: 'Dark',  bg: '#0f0f18', border: 'rgba(255,255,255,0.12)',  accent: '#78a2f4', brand: 'rgba(255,255,255,0.82)'  },
  { id: 'steel', label: 'Steel', bg: '#141820', border: 'rgba(148,168,200,0.15)', accent: '#b8cce0', brand: 'rgba(200,215,235,0.82)'  },
  { id: 'light', label: 'Light', bg: '#ffffff', border: 'rgba(0,0,0,0.10)',       accent: '#3a5bd9', brand: 'rgba(20,20,45,0.82)'     },
  { id: 'teal',  label: 'Teal',  bg: '#0e1a1d', border: 'rgba(91,184,196,0.14)',  accent: '#5bb8c4', brand: 'rgba(190,225,230,0.82)'  },
]

function MiniPill({ theme }: { theme: PillThemeDef }) {
  return (
    <div
      style={{
        height: 22,
        borderRadius: 999,
        display: 'flex',
        alignItems: 'center',
        gap: 4,
        padding: '0 7px 0 6px',
        background: theme.bg,
        border: `1px solid ${theme.border}`,
        boxShadow: theme.bg === '#ffffff'
          ? '0 1px 6px rgba(0,0,0,0.10)'
          : 'inset 0 1px 0 rgba(255,255,255,0.04)',
        flexShrink: 0,
      }}
    >
      <svg width="7" height="7" viewBox="0 0 24 24" fill="none" stroke={theme.accent} strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M12 2a3 3 0 0 1 3 3v7a3 3 0 0 1-6 0V5a3 3 0 0 1 3-3z"/>
        <path d="M19 10a7 7 0 0 1-14 0"/>
        <line x1="12" y1="19" x2="12" y2="22"/>
        <line x1="9" y1="22" x2="15" y2="22"/>
      </svg>
      <span style={{ fontSize: 8, fontWeight: 600, color: theme.brand, letterSpacing: '0.01em', whiteSpace: 'nowrap' }}>
        NexusVoice
      </span>
    </div>
  )
}

export const PillTab = memo(function PillTab() {
  const { pillTheme, setPillTheme } = useAppStore()

  const handleSelect = (id: PillTheme) => {
    setPillTheme(id)
    void emit(EVENTS.PILL_THEME_CHANGED, id)
  }

  return (
    <div className="flex flex-col gap-5">
      <div>
        <p className="text-[11px] font-semibold text-[var(--fg-2)] uppercase tracking-[0.03em] mb-3">Pill Appearance</p>
        <p className="text-[12px] text-[var(--muted)] mb-4">Choose a color theme for the floating pill overlay.</p>

        <div className="grid gap-2" style={{ gridTemplateColumns: 'repeat(4, minmax(100px, 160px))' }}>
          {PILL_THEMES.map((t) => {
            const active = pillTheme === t.id
            return (
              <motion.button
                key={t.id}
                type="button"
                className="flex flex-col p-0 rounded-[var(--r-md)] border-[1.5px] cursor-pointer text-left overflow-hidden"
                onClick={() => handleSelect(t.id)}
                initial={false}
                animate={{
                  borderColor: active ? 'var(--accent)' : 'var(--border-soft)',
                  boxShadow: active ? '0 0 0 1px var(--accent)' : '0 0 0 0px transparent',
                  backgroundColor: 'var(--surface)',
                }}
                whileHover={{ y: -1, boxShadow: active ? '0 0 0 1px var(--accent)' : 'var(--shadow-md)' }}
                whileTap={{ scale: 0.99 }}
                transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
              >
                {/* Preview area */}
                <div
                  className="w-full flex flex-col items-center justify-center gap-2 py-4 px-3"
                  style={{
                    background: t.bg === '#ffffff' ? '#e8eaf0' : `color-mix(in srgb, ${t.bg} 60%, #111)`,
                    borderBottom: '1px solid var(--border-soft)',
                    minHeight: 80,
                  }}
                >
                  <MiniPill theme={t} />
                </div>

                {/* Label row */}
                <div className="flex items-center justify-between px-[6px] py-[4px]">
                  <motion.span
                    className="text-[10px] font-semibold tracking-[-0.01em]"
                    initial={false}
                    animate={{ color: active ? 'var(--accent)' : 'var(--fg)' }}
                    transition={{ duration: 0.2 }}
                  >
                    {t.label}
                  </motion.span>
                  <AnimatePresence>
                    {active && (
                      <motion.span
                        initial={{ scale: 0, opacity: 0 }}
                        animate={{ scale: 1, opacity: 1 }}
                        exit={{ scale: 0, opacity: 0 }}
                        transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
                      >
                        <Check size={8} strokeWidth={3.5} className="text-[var(--accent)] flex-shrink-0" />
                      </motion.span>
                    )}
                  </AnimatePresence>
                </div>
              </motion.button>
            )
          })}
        </div>
      </div>
    </div>
  )
})
