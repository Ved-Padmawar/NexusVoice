import { memo } from 'react'
import { Check, Moon, Sun } from 'lucide-react'
import { useAppStore } from '../../store/useAppStore'
import { THEMES, type ThemeMeta } from '../../lib/themes'
import { Section } from '../../components/page'
import { PillTab } from './PillTab'
import { WaveformTab } from './WaveformTab'

/** The window in miniature, drawn by the previewed theme's own tokens. */
function ThemePreview({ name }: { name: ThemeMeta['name'] }) {
  return (
    <div className="nv-mini" data-theme={name} aria-hidden>
      <div className="nv-mini__bar">
        <i className="nv-mini__logo" />
        <div className="nv-mini__nav"><i /><i /><i /></div>
      </div>
      <div className="nv-mini__hero">
        <div className="nv-mini__keys">
          <i className="nv-mini__key" />
          <i className="nv-mini__key nv-mini__key--wide" />
        </div>
        <i className="nv-mini__line" />
        <i className="nv-mini__line nv-mini__line--short" />
        <i className="nv-mini__cta" />
      </div>
    </div>
  )
}

function ThemeCard({ t, active, onPick }: { t: ThemeMeta; active: boolean; onPick: () => void }) {
  return (
    <button type="button" aria-pressed={active} onClick={onPick} className="nv-choice">
      <span className="nv-choice__stage"><ThemePreview name={t.name} /></span>
      <span className="nv-choice__foot">
        <span className="min-w-0">
          <span className="nv-choice__label block">{t.label}</span>
          <span className="nv-choice__hint block">{t.blurb}</span>
        </span>
        {active && <span className="nv-choice__check"><Check strokeWidth={3} /></span>}
      </span>
    </button>
  )
}

export const AppearanceTab = memo(function AppearanceTab() {
  const theme = useAppStore(s => s.theme)
  const setTheme = useAppStore(s => s.setTheme)

  return (
    <>
      <Section title="Window theme" description="The colours of this window.">
        {([
          { mode: 'dark' as const, label: 'Dark', Icon: Moon },
          { mode: 'light' as const, label: 'Light', Icon: Sun },
        ]).map(({ mode, label, Icon }) => (
          <div key={mode} className="mt-4 first:mt-0">
            <h3 className="mb-2.5 flex items-center gap-1.5 text-[12.5px] font-semibold text-fg-2">
              <Icon size={13} strokeWidth={2} className="text-muted" />{label}
            </h3>
            <div className="nv-choice-grid">
              {THEMES.filter(t => t.mode === mode).map(t => (
                <ThemeCard key={t.name} t={t} active={theme === t.name} onPick={() => setTheme(t.name)} />
              ))}
            </div>
          </div>
        ))}
      </Section>

      <PillTab />
      <WaveformTab />
    </>
  )
})
