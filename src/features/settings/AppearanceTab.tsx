import { useShallow } from 'zustand/react/shallow'
import { memo } from 'react'
import { useAppStore, type ThemeName } from '../../app/store/useAppStore'
import { Swatch } from '../../shared/ui/Swatch'

type ThemeDef = {
  name: ThemeName
  label: string
  blurb: string
  mode: 'dark' | 'light'
  bg: string
  panel: string
  accent: string
  border: string
  surface: string
  muted: string
  fg: string
}

const THEMES: ThemeDef[] = [
  { name: 'steel',    label: 'Steel',    blurb: 'Graphite with brass',    mode: 'dark',  bg: '#0f131a', panel: '#151a23', accent: '#b9cde2', border: '#2c3646', surface: '#1c232e', muted: '#8a97ab', fg: '#e4eaf3' },
  { name: 'midnight', label: 'Midnight', blurb: 'Ink navy and cyan',      mode: 'dark',  bg: '#05080e', panel: '#0b0f17', accent: '#22d3d3', border: '#253044', surface: '#121824', muted: '#8992a6', fg: '#edf0f6' },
  { name: 'indigo',   label: 'Indigo',   blurb: 'Blue-violet night',      mode: 'dark',  bg: '#0e0f1f', panel: '#141629', accent: '#8b93f8', border: '#2f3356', surface: '#1b1e35', muted: '#959bc0', fg: '#eceefb' },
  { name: 'orchid',   label: 'Orchid',   blurb: 'Aubergine and orchid',   mode: 'dark',  bg: '#120e19', panel: '#191423', accent: '#ec84f7', border: '#3a2f4e', surface: '#221b2f', muted: '#a093b6', fg: '#f1ebf8' },
  { name: 'canvas',   label: 'Canvas',   blurb: 'Cool paper, cobalt',     mode: 'light', bg: '#e9edf4', panel: '#ffffff', accent: '#3456d6', border: '#c9d0dd', surface: '#f4f6fa', muted: '#596279', fg: '#141927' },
  { name: 'dawn',     label: 'Dawn',     blurb: 'Parchment and orange',   mode: 'light', bg: '#f2e3d3', panel: '#fffaf3', accent: '#b9500a', border: '#e0c7ab', surface: '#fbefe2', muted: '#7a604c', fg: '#2b1d12' },
  { name: 'breeze',   label: 'Breeze',   blurb: 'Sea glass and teal',     mode: 'light', bg: '#d9eaee', panel: '#f7fbfc', accent: '#0c6f82', border: '#b6d2d8', surface: '#e9f3f5', muted: '#4b6a73', fg: '#0f2a33' },
  { name: 'blossom',  label: 'Blossom',  blurb: 'Rose quartz, crimson',   mode: 'light', bg: '#f1dde2', panel: '#fff8f9', accent: '#b02a49', border: '#e3bfc8', surface: '#fbedf0', muted: '#7a5662', fg: '#2e1720' },
]

function ThemePreview({ t }: { t: ThemeDef }) {
  return (
    <div className="relative h-full w-full overflow-hidden rounded-md" style={{ background: t.bg }}>
      <div className="absolute inset-y-0 left-0 w-13" style={{ background: t.panel, borderRight: `1px solid ${t.border}` }}>
        <div className="flex items-center gap-1.5 px-2 pt-2.5">
          <div className="size-1.75 rounded-xs" style={{ background: t.accent }} />
          <div className="h-0.75 w-6 rounded-full opacity-70" style={{ background: t.muted }} />
        </div>
        <div className="mt-3.5 flex flex-col gap-1.25 px-1.5">
          <div className="flex items-center gap-1.5 rounded-[3px] px-1.5 py-1" style={{ background: `${t.accent}22` }}>
            <div className="size-1.25 rounded-[1px]" style={{ background: t.accent }} />
            <div className="h-0.75 w-5.5 rounded-full" style={{ background: t.accent }} />
          </div>
          {[0, 1].map((i) => (
            <div key={i} className="flex items-center gap-1.5 px-1.5 py-1">
              <div className="size-1.25 rounded-[1px] opacity-45" style={{ background: t.muted }} />
              <div className="h-0.75 w-4.5 rounded-full opacity-35" style={{ background: t.muted }} />
            </div>
          ))}
        </div>
        <div className="absolute inset-x-1.5 bottom-2 flex items-center gap-1.5 rounded-[3px] px-1.5 py-1" style={{ background: t.surface }}>
          <div className="size-2 rounded-full opacity-60" style={{ background: t.accent }} />
          <div className="h-0.75 w-5 rounded-full opacity-40" style={{ background: t.muted }} />
        </div>
      </div>

      <div className="absolute inset-y-0 left-13 right-0">
        <div className="flex h-3.5 items-center justify-end gap-1.5 pr-2">
          <div className="h-px w-1.5" style={{ background: t.muted, opacity: 0.6 }} />
          <div className="size-1 border" style={{ borderColor: t.muted, opacity: 0.6 }} />
          <div className="size-1 rotate-45 border-l border-t" style={{ borderColor: t.muted, opacity: 0.6 }} />
        </div>

        <div className="flex items-center gap-2 px-2.5 pb-2">
          <div className="min-w-0">
            <div className="h-1 w-10 rounded-full opacity-90" style={{ background: t.fg }} />
            <div className="mt-1 h-0.75 w-14 rounded-full opacity-40" style={{ background: t.muted }} />
          </div>
          <div className="ml-auto h-2.5 w-11 rounded-[3px]" style={{ background: t.surface, border: `1px solid ${t.border}` }} />
        </div>

        <div className="flex gap-1.5 px-2.5">
          <div className="flex flex-1 flex-col gap-1.5">
            {[100, 84, 94].map((w, i) => (
              <div key={i} className="rounded-[3px] px-1.5 py-1.25" style={{ background: t.panel }}>
                <div className="h-0.75 rounded-full opacity-65" style={{ background: t.fg, width: `${w}%` }} />
                <div className="mt-1 h-0.75 w-[56%] rounded-full opacity-30" style={{ background: t.muted }} />
              </div>
            ))}
          </div>
          <div className="flex w-15 shrink-0 flex-col gap-1.5">
            <div className="rounded-[3px] p-1.5" style={{ background: t.panel }}>
              <div className="h-1.5 w-7 rounded-full opacity-85" style={{ background: t.fg }} />
              <div className="mt-1.25 h-2 w-full rounded-xs opacity-30" style={{ background: t.accent }} />
            </div>
            <div className="rounded-[3px] p-1.5" style={{ background: `${t.accent}1f`, border: `1px solid ${t.accent}44` }}>
              <div className="h-0.75 w-6 rounded-full" style={{ background: t.accent }} />
              <div className="mt-1 h-0.75 w-8 rounded-full opacity-50" style={{ background: t.accent }} />
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

function ThemeStrip({ t }: { t: ThemeDef }) {
  return (
    <span className="flex shrink-0 -space-x-1.5" aria-hidden="true">
      {[t.bg, t.surface, t.accent, t.muted].map((c, i) => (
        <span
          key={i}
          className="size-5 rounded-full"
          style={{ background: c, boxShadow: i < 2 ? 'inset 0 0 0 1px color-mix(in oklab, var(--fg) 25%, transparent)' : '0 0 0 2px var(--panel)' }}
        />
      ))}
    </span>
  )
}

/** Picking a theme is a before/after judgement, so the preview holds its
 *  position and only its colours move. */
export const AppearanceTab = memo(function AppearanceTab() {
  const { theme, setTheme } = useAppStore(useShallow(s => ({
    theme: s.theme,
    setTheme: s.setTheme,
  })))
  const current = THEMES.find((t) => t.name === theme) ?? THEMES[0]

  return (
    <section>
    <h2 className="mb-2 text-[13px] font-semibold text-fg-2">Window theme</h2>
    <div className="flex gap-4 rounded-lg border border-line-soft bg-panel p-4">
      <div className="w-55 shrink-0 overflow-hidden rounded-md border border-line-soft shadow-(--shadow-sm)">
        <ThemePreview t={current} />
      </div>

      {/* Dark themes down the left column, light down the right. */}
      <div className="grid min-w-0 flex-1 grid-flow-col grid-cols-2 grid-rows-4 gap-1.5">
        {THEMES.map((t) => (
          <Swatch
            key={t.name}
            inline
            preview={<ThemeStrip t={t} />}
            label={t.label}
            hint={t.blurb}
            active={theme === t.name}
            onPick={() => setTheme(t.name)}
          />
        ))}
      </div>
    </div>
    </section>
  )
})
