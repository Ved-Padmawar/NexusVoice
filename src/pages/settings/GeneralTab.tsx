import { memo } from 'react'
import { Check } from 'lucide-react'
import { useAppStore, type ThemeName } from '../../store/useAppStore'

const THEMES: {
  name: ThemeName; label: string; mode: 'dark' | 'light'
  bg: string; panel: string; accent: string; border: string; surface: string; muted: string
}[] = [
  { name: 'abyss',    label: 'Abyss',    mode: 'dark',  bg: '#22232b', panel: '#2a2b35', accent: '#78a2f4', border: '#383a48', surface: '#2e303d', muted: '#6a6e88' },
  { name: 'midnight', label: 'Midnight', mode: 'dark',  bg: '#0d101c', panel: '#131526', accent: '#8b5cf6', border: '#1e2240', surface: '#181b2e', muted: '#4a507a' },
  { name: 'nebula',   label: 'Nebula',   mode: 'dark',  bg: '#231b2a', panel: '#2c2433', accent: '#9d38a8', border: '#3e3048', surface: '#332840', muted: '#6a5878' },
  { name: 'pine',     label: 'Pine',     mode: 'dark',  bg: '#1b2420', panel: '#222d29', accent: '#58c596', border: '#304038', surface: '#283530', muted: '#507060' },
  { name: 'canvas',   label: 'Canvas',   mode: 'light', bg: '#f8f9fc', panel: '#ffffff', accent: '#3a5bd9', border: '#d8dce8', surface: '#f0f2f8', muted: '#8890b0' },
  { name: 'dawn',     label: 'Dawn',     mode: 'light', bg: '#faf4ee', panel: '#ede0d0', accent: '#d4610a', border: '#d8c8b4', surface: '#f5ede2', muted: '#9a8870' },
  { name: 'breeze',   label: 'Breeze',   mode: 'light', bg: '#eef6f8', panel: '#d8eef0', accent: '#1a7a8a', border: '#c0d8dc', surface: '#e8f4f6', muted: '#6a9098' },
  { name: 'blossom',  label: 'Blossom',  mode: 'light', bg: '#f8eef0', panel: '#e8d4d8', accent: '#c0304a', border: '#d8c0c4', surface: '#f2e4e8', muted: '#9a7078' },
]

function ThemePreview({ bg, panel, accent, border, surface, muted }: {
  bg: string; panel: string; accent: string; border: string; surface: string; muted: string
}) {
  return (
    <svg viewBox="0 0 120 72" xmlns="http://www.w3.org/2000/svg" className="w-full block rounded-t-[var(--r-md)]">
      <rect width="120" height="72" fill={bg} />
      <rect x="0" y="0" width="120" height="9" fill={panel} />
      <circle cx="6" cy="4.5" r="1.8" fill={muted} opacity="0.6" />
      <circle cx="11" cy="4.5" r="1.8" fill={muted} opacity="0.6" />
      <circle cx="16" cy="4.5" r="1.8" fill={muted} opacity="0.6" />
      <rect x="42" y="3" width="36" height="3" rx="1.5" fill={border} opacity="0.7" />
      <rect x="0" y="9" width="28" height="63" fill={panel} />
      <rect x="0" y="9" width="28" height="63" fill="none" stroke={border} strokeWidth="0.5" />
      <rect x="4" y="16" width="3" height="3" rx="1" fill={accent} opacity="0.9" />
      <rect x="10" y="17" width="14" height="2" rx="1" fill={accent} opacity="0.5" />
      <rect x="4" y="24" width="3" height="3" rx="1" fill={muted} opacity="0.5" />
      <rect x="10" y="25" width="12" height="2" rx="1" fill={muted} opacity="0.3" />
      <rect x="4" y="32" width="3" height="3" rx="1" fill={muted} opacity="0.5" />
      <rect x="10" y="33" width="10" height="2" rx="1" fill={muted} opacity="0.3" />
      <rect x="32" y="14" width="22" height="3" rx="1.5" fill={muted} opacity="0.5" />
      <rect x="32" y="22" width="20" height="12" rx="2" fill={surface} stroke={border} strokeWidth="0.5" />
      <rect x="55" y="22" width="20" height="12" rx="2" fill={surface} stroke={border} strokeWidth="0.5" />
      <rect x="78" y="22" width="20" height="12" rx="2" fill={surface} stroke={border} strokeWidth="0.5" />
      <rect x="101" y="22" width="15" height="12" rx="2" fill={surface} stroke={border} strokeWidth="0.5" />
      <rect x="35" y="28" width="14" height="3" rx="1" fill={accent} opacity="0.85" />
      <rect x="58" y="28" width="14" height="3" rx="1" fill={accent} opacity="0.6" />
      <rect x="81" y="28" width="14" height="3" rx="1" fill={accent} opacity="0.4" />
      <rect x="104" y="28" width="9" height="3" rx="1" fill={accent} opacity="0.25" />
      <rect x="32" y="38" width="84" height="28" rx="2" fill={surface} stroke={border} strokeWidth="0.5" />
      <rect x="36" y="43" width="40" height="2" rx="1" fill={muted} opacity="0.4" />
      <rect x="36" y="48" width="60" height="2" rx="1" fill={muted} opacity="0.25" />
      <rect x="36" y="53" width="50" height="2" rx="1" fill={muted} opacity="0.2" />
      <rect x="36" y="58" width="30" height="2" rx="1" fill={accent} opacity="0.35" />
    </svg>
  )
}

export const GeneralTab = memo(function GeneralTab() {
  const { theme, setTheme } = useAppStore()

  return (
    <div className="flex flex-col gap-5">
      <div>
        <p className="text-[11px] font-semibold text-[var(--fg-2)] uppercase tracking-[0.03em] mb-3">Appearance</p>
        <p className="text-[12px] text-[var(--muted)] mb-4">Choose a color scheme for your workspace.</p>

        {(['dark', 'light'] as const).map((mode) => {
          const group = THEMES.filter(t => t.mode === mode)
          return (
            <div key={mode} className="mb-5 last:mb-0">
              <p className="text-[10px] font-semibold text-[var(--muted)] uppercase tracking-[0.06em] mb-2">{mode === 'dark' ? 'Dark' : 'Light'}</p>
              <div className="grid gap-2" style={{ gridTemplateColumns: 'repeat(4, 120px)' }}>
                {group.map((t) => {
                  const active = theme === t.name
                  return (
                    <button
                      key={t.name}
                      type="button"
                      className={`flex flex-col p-0 rounded-[var(--r-md)] border-[1.5px] bg-[var(--surface)] cursor-pointer text-left overflow-hidden transition-all duration-[var(--t-fast)] hover:-translate-y-px hover:shadow-[var(--shadow-md)] ${active ? 'border-[var(--accent)] shadow-[0_0_0_1px_var(--accent)]' : 'border-[var(--border-soft)] hover:border-[var(--border)]'}`}
                      onClick={() => setTheme(t.name)}
                    >
                      <ThemePreview bg={t.bg} panel={t.panel} accent={t.accent} border={t.border} surface={t.surface} muted={t.muted} />
                      <div className="flex items-center justify-between px-[6px] py-[4px]">
                        <span className={`text-[10px] font-semibold tracking-[-0.01em] ${active ? 'text-[var(--accent)]' : 'text-[var(--fg)]'}`}>{t.label}</span>
                        {active && <Check size={8} strokeWidth={3.5} className="text-[var(--accent)] flex-shrink-0" />}
                      </div>
                    </button>
                  )
                })}
              </div>
            </div>
          )
        })}

      </div>
    </div>
  )
})
