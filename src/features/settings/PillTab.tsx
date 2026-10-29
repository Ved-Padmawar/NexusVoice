import { memo } from 'react'
import { Swatch } from '../../shared/ui/Swatch'
import { PILL_THEMES, type PillThemeDef } from '../../pill/pillThemes'
import type { PillTheme } from '../../shared/types'
import { updatePill, usePillSettings } from './queries'

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

/** Same strip as the window swatches. The translucent values sit on the
 *  pill's own ground, which is how they actually appear. */
function PillStrip({ t }: { t: PillThemeDef }) {
  return (
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
  )
}

/** The pill floats over other apps, so it is themed separately. */
export const PillTab = memo(function PillTab() {
  const pillTheme = usePillSettings().theme
  const current = PILL_THEMES.find((t) => t.id === pillTheme) ?? PILL_THEMES[0]

  const handleSelect = (id: PillTheme) => { void updatePill({ theme: id }) }

  return (
    <section>
    <h2 className="mb-2 text-[13px] font-semibold text-fg-2">Recording pill</h2>
    <div className="flex gap-4 rounded-lg border border-line-soft bg-panel p-4">
      <div className="flex shrink-0 flex-col gap-2.5">
        {/* Backdrop mixed from the pill's own ground, so it has something to
            sit off without a pattern competing with it. */}
        <div
          className="grid h-21.5 w-55 place-items-center rounded-md border border-line-soft shadow-(--shadow-sm)"
          style={{ background: current.stage }}
        >
          <MiniPill theme={current} />
        </div>
        <span className="text-[11px] text-muted">Shown while you dictate</span>
      </div>

      <div className="grid min-w-0 flex-1 grid-cols-3 content-center gap-1.5">
        {PILL_THEMES.map((t) => (
          <Swatch
            key={t.id}
            preview={<PillStrip t={t} />}
            label={t.label}
            hint={t.blurb}
            active={pillTheme === t.id}
            onPick={() => handleSelect(t.id)}
          />
        ))}
      </div>
    </div>
    </section>
  )
})
