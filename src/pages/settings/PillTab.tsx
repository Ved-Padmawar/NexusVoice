import { useShallow } from 'zustand/react/shallow'
import { memo } from 'react'
import { emit } from '@tauri-apps/api/event'
import { Check } from 'lucide-react'
import { useAppStore } from '../../store/useAppStore'
import { EVENTS } from '../../lib/events'
import { PILL_THEMES, pillBackdrop, type PillThemeDef } from '../../lib/pillThemes'
import type { PillTheme } from '../../store/uiSlice'
import { Section } from '../../components/page'

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

/** The pill floats over other apps, so it is themed separately. */
export const PillTab = memo(function PillTab() {
  const { pillTheme, setPillTheme } = useAppStore(useShallow(s => ({
    pillTheme: s.pillTheme,
    setPillTheme: s.setPillTheme,
  })))

  const handleSelect = (id: PillTheme) => {
    setPillTheme(id)
    void emit(EVENTS.PILL_THEME_CHANGED, id)
  }

  return (
    <Section title="Recording pill" description="The pill floats over whatever you are typing in, so it keeps a look of its own.">
      <div className="nv-choice-grid">
        {PILL_THEMES.map((t) => {
          const active = pillTheme === t.id
          return (
            <button key={t.id} type="button" aria-pressed={active} onClick={() => handleSelect(t.id)} className="nv-choice">
              <span className="nv-choice__stage" style={{ background: pillBackdrop(t) }}>
                <span className="inline-flex scale-[1.6]"><MiniPill theme={t} /></span>
              </span>
              <span className="nv-choice__foot">
                <span className="nv-choice__label">{t.label}</span>
                {active && <span className="nv-choice__check"><Check strokeWidth={3} /></span>}
              </span>
            </button>
          )
        })}
      </div>
    </Section>
  )
})
