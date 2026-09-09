import { useShallow } from 'zustand/react/shallow'
import { memo } from 'react'
import { emit } from '@tauri-apps/api/event'
import { Check, Radio } from 'lucide-react'
import { useAppStore } from '../../store/useAppStore'
import { EVENTS } from '../../lib/events'
import { PILL_THEMES, type PillThemeDef } from '../../lib/pillThemes'
import { Section } from '../../components/Section'
import type { PillTheme } from '../../store/uiSlice'

/** The pill floats over other apps, so it carries its own palette. These are
 *  fixed values from the pill's stylesheet, not window-theme tokens. */
function MiniPill({ theme }: { theme: PillThemeDef }) {
  return (
    <div
      style={{
        height: 24,
        borderRadius: 999,
        display: 'flex',
        alignItems: 'center',
        gap: 5,
        padding: '0 8px 0 7px',
        background: theme.bg,
        border: `1px solid ${theme.border}`,
        boxShadow: theme.bg === '#ffffff'
          ? '0 1px 6px rgba(0,0,0,0.10)'
          : 'inset 0 1px 0 rgba(255,255,255,0.04)',
        flexShrink: 0,
      }}
    >
      <svg width="8" height="8" viewBox="0 0 24 24" fill="none" stroke={theme.accent} strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M12 2a3 3 0 0 1 3 3v7a3 3 0 0 1-6 0V5a3 3 0 0 1 3-3z" />
        <path d="M19 10a7 7 0 0 1-14 0" />
        <line x1="12" y1="19" x2="12" y2="22" />
        <line x1="9" y1="22" x2="15" y2="22" />
      </svg>
      <span style={{ fontSize: 8.5, fontWeight: 600, color: theme.brand, whiteSpace: 'nowrap' }}>
        NexusVoice
      </span>
    </div>
  )
}

function PillCard({ t, active, onPick }: { t: PillThemeDef; active: boolean; onPick: () => void }) {
  return (
    <button type="button" onClick={onPick} aria-pressed={active} className="pick flex flex-col gap-2 p-2">
      {/* The pill sits on a ground mixed from its own background, because that
          is roughly what it floats over in use. */}
      <span
        className="grid h-14 w-full place-items-center rounded-(--r-md)"
        style={{
          background: t.bg === '#ffffff' ? '#e8eaf0' : `color-mix(in srgb, ${t.bg} 55%, #101215)`,
        }}
      >
        <MiniPill theme={t} />
      </span>
      <span className="flex w-full items-center gap-1 px-0.5">
        <span className={`min-w-0 flex-1 truncate text-left text-[11.5px] font-semibold ${active ? 'text-(--on-soft)' : 'text-(--fg)'}`}>
          {t.label}
        </span>
        {active && (
          <span className="grid size-3.5 shrink-0 place-items-center rounded-full bg-(--accent)">
            <Check size={8} strokeWidth={3.5} className="text-(--accent-fg)" />
          </span>
        )}
      </span>
    </button>
  )
}

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
    <Section
      title="Recording pill"
      Icon={Radio}
      description="Floats over other windows while you dictate, so it is themed on its own."
      bodyClassName="grid grid-cols-4 gap-2.5 p-3.5"
    >
      {PILL_THEMES.map((t) => (
        <PillCard key={t.id} t={t} active={pillTheme === t.id} onPick={() => handleSelect(t.id)} />
      ))}
    </Section>
  )
})
