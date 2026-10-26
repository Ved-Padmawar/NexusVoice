import { useShallow } from 'zustand/react/shallow'
import { memo } from 'react'
import { motion } from 'framer-motion'
import { Check } from 'lucide-react'
import { useAppStore } from '../../store/useAppStore'
import { PILL_THEMES, type PillThemeDef } from '../../lib/pillThemes'
import type { PillTheme } from '../../types'

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
        flexShrink: 0,
      }}
    >
      <svg width="7" height="7" viewBox="0 0 24 24" fill="none" stroke={theme.accent} strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M12 2a3 3 0 0 1 3 3v7a3 3 0 0 1-6 0V5a3 3 0 0 1 3-3z" />
        <path d="M19 10a7 7 0 0 1-14 0" />
        <line x1="12" y1="19" x2="12" y2="22" />
        <line x1="9" y1="22" x2="15" y2="22" />
      </svg>
      <span style={{ fontSize: 8, fontWeight: 600, color: theme.brand, letterSpacing: '0.01em', whiteSpace: 'nowrap' }}>
        NexusVoice
      </span>
    </div>
  )
}

/** The swatch is a small pill: the shape is most of what you are choosing. */
function PillSwatch({ t, active, onPick }: { t: PillThemeDef; active: boolean; onPick: () => void }) {
  return (
    <motion.button
      type="button"
      onClick={onPick}
      aria-pressed={active}
      className="flex h-10 items-center gap-2.5 rounded-(--r-md) border px-2.5 text-left cursor-pointer"
      initial={false}
      animate={{
        borderColor: active ? 'var(--accent)' : 'var(--border-soft)',
        backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface)',
      }}
      whileHover={active ? undefined : { borderColor: 'var(--border)', backgroundColor: 'var(--surface-hover)' }}
      whileTap={{ scale: 0.99 }}
      transition={{ duration: 0.15 }}
    >
      {/* Same strip as the window swatches. The translucent values sit on the
          pill's own ground, which is how they actually appear. */}
      <span className="flex shrink-0 -space-x-1.5" aria-hidden="true">
        {[t.bg, t.border, t.brand, t.accent].map((c, i) => (
          <span
            key={i}
            className="size-5 rounded-full"
            style={{
              background: `linear-gradient(${c}, ${c}), ${t.bg}`,
              boxShadow: i < 2 ? 'inset 0 0 0 1px color-mix(in oklab, var(--fg) 25%, transparent)' : '0 0 0 2px var(--panel)',
            }}
          />
        ))}
      </span>

      <span className="min-w-0 flex-1">
        <motion.span
          className="block truncate text-[12px] font-semibold tracking-[-0.01em]"
          initial={false}
          animate={{ color: active ? 'var(--accent)' : 'var(--fg)' }}
          transition={{ duration: 0.2 }}
        >
          {t.label}
        </motion.span>
        <span className="block truncate text-[10px] text-muted-foreground">{t.blurb}</span>
      </span>

      {active && (
        <span className="grid size-4 shrink-0 place-items-center rounded-full bg-(--accent)">
          <Check size={9} strokeWidth={3.5} className="text-primary-foreground" />
        </span>
      )}
    </motion.button>
  )
}

/** The pill floats over other apps, so it is themed separately. */
export const PillTab = memo(function PillTab() {
  const { pillTheme, updatePill } = useAppStore(useShallow(s => ({
    pillTheme: s.pill.theme,
    updatePill: s.updatePill,
  })))
  const current = PILL_THEMES.find((t) => t.id === pillTheme) ?? PILL_THEMES[0]

  const handleSelect = (id: PillTheme) => { void updatePill({ theme: id }) }

  return (
    <section>
    <h2 className="mb-2 text-[13px] font-semibold text-(--fg-2)">Recording pill</h2>
    <div className="flex gap-4 rounded-(--r-lg) border border-(--border-soft) bg-(--panel) p-4">
      <div className="flex shrink-0 flex-col gap-2.5">
        {/* Backdrop mixed from the pill's own ground, so it has something to
            sit off without a pattern competing with it. */}
        <div
          className="grid h-21.5 w-55 place-items-center rounded-(--r-md) border border-(--border-soft) shadow-(--shadow-sm)"
          style={{ background: current.stage }}
        >
          <MiniPill theme={current} />
        </div>
        <span className="text-[11px] text-muted-foreground">Shown while you dictate</span>
      </div>

      <div className="grid min-w-0 flex-1 grid-cols-3 content-center gap-1.5">
        {PILL_THEMES.map((t) => (
          <PillSwatch
            key={t.id}
            t={t}
            active={pillTheme === t.id}
            onPick={() => handleSelect(t.id)}
          />
        ))}
      </div>
    </div>
    </section>
  )
})
