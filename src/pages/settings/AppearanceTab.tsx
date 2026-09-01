import { memo } from 'react'
import { motion } from 'framer-motion'
import { Check, Moon, Sun } from 'lucide-react'
import { useAppStore, type ThemeName } from '../../store/useAppStore'

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
  { name: 'abyss',    label: 'Abyss',    blurb: 'cool slate, periwinkle accent', mode: 'dark',  bg: '#22232b', panel: '#2a2b35', accent: '#78a2f4', border: '#383a48', surface: '#2e303d', muted: '#6a6e88', fg: '#e6e8f0' },
  { name: 'midnight', label: 'Midnight', blurb: 'slate indigo, cyan accent',     mode: 'dark',  bg: '#080b12', panel: '#11141a', accent: '#1ad1d1', border: '#2e333c', surface: '#1a1d23', muted: '#7a808d', fg: '#eceef4' },
  { name: 'steel',    label: 'Steel',    blurb: 'neutral grey, cool highlight',  mode: 'dark',  bg: '#141820', panel: '#191e28', accent: '#b8cce0', border: '#252d3a', surface: '#1e2530', muted: '#5a6880', fg: '#dfe5ee' },
  { name: 'pine',     label: 'Pine',     blurb: 'forest green, mint accent',     mode: 'dark',  bg: '#1b2420', panel: '#222d29', accent: '#58c596', border: '#304038', surface: '#283530', muted: '#507060', fg: '#dfeae4' },
  { name: 'canvas',   label: 'Canvas',   blurb: 'clean white, cobalt accent',    mode: 'light', bg: '#f8f9fc', panel: '#ffffff', accent: '#3a5bd9', border: '#d8dce8', surface: '#f0f2f8', muted: '#8890b0', fg: '#1a1c28' },
  { name: 'dawn',     label: 'Dawn',     blurb: 'warm paper, amber accent',      mode: 'light', bg: '#faf4ee', panel: '#ede0d0', accent: '#d4610a', border: '#d8c8b4', surface: '#f5ede2', muted: '#9a8870', fg: '#2c2418' },
  { name: 'breeze',   label: 'Breeze',   blurb: 'pale cyan, teal accent',        mode: 'light', bg: '#eef6f8', panel: '#d8eef0', accent: '#1a7a8a', border: '#c0d8dc', surface: '#e8f4f6', muted: '#6a9098', fg: '#16303a' },
  { name: 'blossom',  label: 'Blossom',  blurb: 'soft rose, crimson accent',     mode: 'light', bg: '#f8eef0', panel: '#e8d4d8', accent: '#c0304a', border: '#d8c0c4', surface: '#f2e4e8', muted: '#9a7078', fg: '#33202a' },
]

function ThemePreview({ t }: { t: ThemeDef }) {
  return (
    <div className="relative h-42 w-70 overflow-hidden rounded-(--r-md)" style={{ background: t.bg }}>
      {/* Rail */}
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

      {/* Content */}
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

function ThemeSwatch({ t, active, onPick }: { t: ThemeDef; active: boolean; onPick: () => void }) {
  return (
    <motion.button
      type="button"
      onClick={onPick}
      aria-pressed={active}
      className="flex items-center gap-2.5 rounded-(--r-md) border px-2.5 py-2 text-left cursor-pointer"
      initial={false}
      animate={{
        borderColor: active ? 'var(--accent)' : 'var(--border-soft)',
        backgroundColor: active ? 'var(--accent-soft)' : 'var(--surface)',
      }}
      whileHover={active ? undefined : { borderColor: 'var(--border)', backgroundColor: 'var(--surface-hover)' }}
      whileTap={{ scale: 0.99 }}
      transition={{ duration: 0.15 }}
    >
      <span className="flex shrink-0 overflow-hidden rounded-(--r-xs) border border-(--border-soft)">
        {[t.bg, t.panel, t.surface, t.border, t.muted, t.accent].map((c, i) => (
          <span key={i} className="h-6 w-2.25" style={{ background: c }} />
        ))}
      </span>

      <motion.span
        className="min-w-0 flex-1 truncate text-[12px] font-semibold tracking-[-0.01em]"
        initial={false}
        animate={{ color: active ? 'var(--accent)' : 'var(--fg)' }}
        transition={{ duration: 0.2 }}
      >
        {t.label}
      </motion.span>

      {active && (
        <span className="grid size-4 shrink-0 place-items-center rounded-full bg-(--accent)">
          <Check size={9} strokeWidth={3.5} className="text-primary-foreground" />
        </span>
      )}
    </motion.button>
  )
}

/** Picking a theme is a before/after judgement, so the preview holds its
 *  position and only its colours move. */
export const AppearanceTab = memo(function AppearanceTab() {
  const { theme, setTheme } = useAppStore()
  const current = THEMES.find((t) => t.name === theme) ?? THEMES[0]

  return (
    <div className="flex gap-4 rounded-(--r-lg) border border-(--border-soft) bg-(--panel) p-4">
      <div className="flex shrink-0 flex-col gap-2.5">
        <div className="overflow-hidden rounded-(--r-md) border border-(--border-soft) shadow-(--shadow-sm)">
          <ThemePreview t={current} />
        </div>
        <div className="flex items-baseline gap-2">
          <span className="text-[13px] font-bold tracking-[-0.02em] text-(--fg)">{current.label}</span>
          <span className="min-w-0 truncate text-[11px] text-muted-foreground">{current.blurb}</span>
        </div>
      </div>

      <div className="flex min-w-0 flex-1 flex-col gap-3">
        {([
          { icon: <Moon size={11} />, label: 'Dark', mode: 'dark' as const },
          { icon: <Sun size={11} />, label: 'Light', mode: 'light' as const },
        ]).map((group) => (
          <div key={group.mode} className="flex flex-col gap-1.5">
            <span className="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
              {group.icon}
              {group.label}
            </span>
            <div className="grid grid-cols-2 gap-1.5">
              {THEMES.filter((t) => t.mode === group.mode).map((t) => (
                <ThemeSwatch
                  key={t.name}
                  t={t}
                  active={theme === t.name}
                  onPick={() => setTheme(t.name)}
                />
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
})
