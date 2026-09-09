import { useShallow } from 'zustand/react/shallow'
import { memo } from 'react'
import { Check, Palette } from 'lucide-react'
import { useAppStore, type ThemeName } from '../../store/useAppStore'
import { Section } from '../../components/Section'

type ThemeDef = {
  name: ThemeName
  label: string
  blurb: string
  mode: 'dark' | 'light'
}

/** Colour values are not repeated here: the theme blocks in `index.css` are
 *  scoped to `[data-theme]`, so a preview simply declares the theme and every
 *  token inside it resolves for real. */
const THEMES: ThemeDef[] = [
  { name: 'abyss',    label: 'Abyss',    blurb: 'Cool slate, even contrast',        mode: 'dark' },
  { name: 'midnight', label: 'Midnight', blurb: 'Near black, vivid cyan',           mode: 'dark' },
  { name: 'steel',    label: 'Steel',    blurb: 'Flat industrial, drawn in lines',  mode: 'dark' },
  { name: 'pine',     label: 'Pine',     blurb: 'Warm green, matte and soft',       mode: 'dark' },
  { name: 'canvas',   label: 'Canvas',   blurb: 'White paper, crisp edges',         mode: 'light' },
  { name: 'dawn',     label: 'Dawn',     blurb: 'Warm paper, printed card',         mode: 'light' },
  { name: 'breeze',   label: 'Breeze',   blurb: 'Cool tint, calm middle',           mode: 'light' },
  { name: 'blossom',  label: 'Blossom',  blurb: 'Soft rose, quiet edges',           mode: 'light' },
]

/** A miniature of the real window in the theme's own tokens — brand strip,
 *  title, readouts, content, dock. What you see is what you get. */
function ThemePreview({ theme }: { theme: ThemeName }) {
  return (
    <div
      data-theme={theme}
      className="relative flex h-40.5 w-full flex-col overflow-hidden rounded-(--r-md) bg-(--bg) shadow-[inset_0_0_0_1px_var(--hairline)]"
    >
      {/* Brand strip: mark, wordmark, status chip. */}
      <div className="flex shrink-0 items-center gap-1.5 px-2.5 pt-2">
        <span className="size-2.5 shrink-0 rounded-[3px] bg-(--accent)" />
        <span className="h-1.5 w-6 rounded-full bg-(--fg-2) opacity-70" />
        <span className="ml-1 h-2.5 w-7 rounded-full bg-(--bg-alt) shadow-[inset_0_0_0_1px_var(--hairline)]" />
        <span className="ml-auto h-1 w-6 rounded-full bg-(--muted) opacity-40" />
      </div>

      <div className="flex min-h-0 flex-1 flex-col px-2.5 pt-3">
        {/* Title row: accent tick, name, description. */}
        <div className="flex items-center gap-1.5">
          <span className="h-3 w-[3px] shrink-0 rounded-full bg-(--accent)" />
          <span className="h-2 w-10 rounded-full bg-(--fg)" />
          <span className="h-1.5 w-12 rounded-full bg-(--muted) opacity-50" />
        </div>

        {/* Readouts on the bare canvas, then the rule. */}
        <div className="mt-3 flex items-end gap-4">
          {[0, 1, 2].map(i => (
            <div key={i} className="flex flex-col gap-1.5">
              <span className="h-3 w-7 rounded-[3px] bg-(--fg)" />
              <span className="h-1.5 w-8 rounded-full bg-(--muted) opacity-60" />
            </div>
          ))}
        </div>
        <span className="mt-2.5 h-px w-full bg-(--hairline)" />

        <div className="mt-2 flex flex-col gap-1.5">
          <span className="h-1.5 w-full rounded-full bg-(--fg-2) opacity-70" />
          <span className="h-1.5 w-2/3 rounded-full bg-(--muted) opacity-60" />
        </div>
      </div>

      {/* The dock, floating. */}
      <div className="absolute bottom-2 left-1/2 flex -translate-x-1/2 items-center gap-1 rounded-full bg-(--rail) p-1 shadow-[inset_0_0_0_1px_var(--border),var(--shadow-md)]">
        <span className="flex h-3.5 items-center gap-1 rounded-full bg-(--nav-on) px-1.5">
          <span className="size-1.5 rounded-full bg-(--nav-on-fg)" />
          <span className="h-1 w-5 rounded-full bg-(--nav-on-fg) opacity-80" />
        </span>
        <span className="mx-0.5 size-1.5 rounded-full bg-(--muted) opacity-60" />
        <span className="mx-0.5 size-1.5 rounded-full bg-(--muted) opacity-60" />
      </div>
    </div>
  )
}

/** A row in the theme list: colour strip, name, blurb, and a tick when active. */
function ThemeRow({ t, active, onPick }: { t: ThemeDef; active: boolean; onPick: () => void }) {
  return (
    <button
      type="button"
      onClick={onPick}
      aria-pressed={active}
      className="pick flex items-center gap-2.5 px-2.5 py-2"
    >
      {/* The strip is drawn from the theme's own tokens, so the swatch is the
          real palette rather than hand-copied hex. */}
      <span
        data-theme={t.name}
        className="flex shrink-0 overflow-hidden rounded-(--r-xs) shadow-[inset_0_0_0_1px_var(--hairline)]"
      >
        <span className="h-5 w-2.5 bg-(--bg)" />
        <span className="h-5 w-2.5 bg-(--panel)" />
        <span className="h-5 w-2.5 bg-(--surface)" />
        <span className="h-5 w-2.5 bg-(--accent)" />
      </span>

      <span className="flex min-w-0 flex-1 flex-col text-left">
        <span className={`truncate text-[12px] font-semibold tracking-[-0.01em] ${active ? 'text-(--on-soft)' : 'text-(--fg)'}`}>
          {t.label}
        </span>
        <span className="truncate text-[10.5px] text-(--muted)">{t.blurb}</span>
      </span>

      {active && (
        <span className="grid size-4 shrink-0 place-items-center rounded-full bg-(--accent)">
          <Check size={9} strokeWidth={3.5} className="text-(--accent-fg)" />
        </span>
      )}
    </button>
  )
}

export const AppearanceTab = memo(function AppearanceTab() {
  const { theme, setTheme } = useAppStore(useShallow(s => ({
    theme: s.theme,
    setTheme: s.setTheme,
  })))

  const current = THEMES.find((t) => t.name === theme) ?? THEMES[0]

  return (
    <Section
      title="Window theme"
      Icon={Palette}
      description="The preview is the real thing, drawn in the selected theme."
      bodyClassName="flex gap-4 p-3.5"
    >
      {/* Held in place so switching themes is a before/after on one surface. */}
      <div className="flex w-64 shrink-0 flex-col gap-2">
        <ThemePreview theme={current.name} />
        <div className="flex items-baseline gap-2 px-0.5">
          <span className="text-[12px] font-semibold tracking-[-0.01em] text-(--fg)">{current.label}</span>
          <span className="min-w-0 truncate text-[10.5px] text-(--muted)">{current.blurb}</span>
        </div>
      </div>

      <div className="grid min-w-0 flex-1 grid-cols-2 content-start gap-1.5">
        {THEMES.map((t) => (
          <ThemeRow key={t.name} t={t} active={theme === t.name} onPick={() => setTheme(t.name)} />
        ))}
      </div>
    </Section>
  )
})
