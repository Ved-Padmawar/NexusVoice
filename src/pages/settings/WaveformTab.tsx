import { memo, useEffect, useRef } from 'react'
import { motion } from 'framer-motion'
import { emit } from '@tauri-apps/api/event'
import { AudioLines, Check } from 'lucide-react'
import { useAppStore } from '../../store/useAppStore'
import { EVENTS } from '../../lib/events'
import { WaveformCanvas } from '../../components/WaveformCanvas'
import { BARS } from '../../lib/waveform'
import { pillThemeDef, type PillThemeDef } from '../../lib/pillThemes'
import type { WaveformStyle } from '../../store/uiSlice'

type StyleDef = {
  id: WaveformStyle
  label: string
  hint: string
}

const WAVEFORM_STYLES: StyleDef[] = [
  { id: 'bars',  label: 'Bars',  hint: 'live level meter' },
  { id: 'memo',  label: 'Memo',  hint: 'scrolling voice note' },
  { id: 'eq',    label: 'EQ',    hint: 'retro equalizer' },
  { id: 'steps', label: 'Steps', hint: 'chunky pixel wave' },
]

const MIN_H = 3
const MAX_H = 16

/**
 * Speech-shaped levels for the preview. The real frames come from the capture
 * thread, which is silent here — random noise would make every style look
 * alike, so this fakes syllables, phonemes and breaths instead.
 */
function usePreviewLevels() {
  const levelsRef = useRef<number[]>(new Array(BARS).fill(0))

  useEffect(() => {
    const smooth = new Array(BARS).fill(0)
    let t = 0
    let phraseEnd = 2.4
    let breathing = false
    let phone = 0
    let phoneEnd = 0
    let vowel = true
    let raf = 0
    let last = performance.now()

    const tick = (now: number) => {
      const dt = Math.min((now - last) / 1000, 0.1)
      last = now
      t += dt

      if (t > phraseEnd) {
        breathing = !breathing
        phraseEnd = t + (breathing ? 0.45 + Math.random() * 0.35 : 1.8 + Math.random() * 2.2)
      }

      let target: number[]
      if (breathing) {
        target = new Array(BARS).fill(0).map(() => Math.random() * 0.03)
      } else {
        // A vowel loads the low bands, a consonant the highs — that split is
        // what makes the bands move independently.
        if (t > phoneEnd) {
          vowel = Math.random() > 0.42
          phone = Math.random()
          phoneEnd = t + (vowel ? 0.09 + Math.random() * 0.13 : 0.04 + Math.random() * 0.06)
        }
        const centre = vowel ? 0.6 + phone * 1.9 : 4.4 + phone * 2.6
        const width = vowel ? 1.5 : 2.1
        const gain = vowel ? 0.95 : 0.62
        const word = 0.6 + 0.4 * Math.sin(t * 1.7 + 0.8)

        target = Array.from({ length: BARS }, (_, i) => {
          const band = Math.exp(-Math.pow((i - centre) / width, 2))
          const jit = 0.78 + Math.random() * 0.44
          return Math.min(1, band * gain * word * jit * 1.5)
        })
      }

      for (let i = 0; i < BARS; i++) {
        const rising = target[i] > smooth[i]
        smooth[i] += (target[i] - smooth[i]) * (rising ? 0.55 : 0.12 + i * 0.008)
      }
      levelsRef.current = smooth.slice()
      raf = requestAnimationFrame(tick)
    }

    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [])

  return levelsRef
}

/** The `bars` style is DOM-drawn, so the preview mirrors that rather than
 *  reimplementing it on canvas — what you see is what the pill renders. */
function BarsPreview({ levelsRef, accent }: { levelsRef: React.RefObject<number[]>; accent: string }) {
  const wrapRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    let raf = 0
    const tick = () => {
      const el = wrapRef.current
      if (el) {
        const levels = levelsRef.current
        const spans = el.children
        for (let i = 0; i < spans.length; i++) {
          const v = levels[i] ?? 0
          ;(spans[i] as HTMLElement).style.height =
            `${Math.max(MIN_H, Math.round(MIN_H + (MAX_H - MIN_H) * v))}px`
        }
      }
      raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [levelsRef])

  return (
    <div ref={wrapRef} className="pill__waveform">
      {Array.from({ length: BARS }, (_, i) => (
        <span key={i} className="pill__bar" style={{ height: MIN_H, background: accent }} />
      ))}
    </div>
  )
}

/** The pill at true recording size (80px), in the user's pill theme. */
function PreviewPill({ style, levelsRef, theme }: {
  style: WaveformStyle
  levelsRef: React.RefObject<number[]>
  theme: PillThemeDef
}) {
  return (
    <div
      className="pill"
      style={{
        width: 80,
        background: theme.bg,
        borderColor: theme.border,
        boxShadow: theme.bg === '#ffffff'
          ? '0 2px 12px rgba(0,0,0,0.10), 0 1px 3px rgba(0,0,0,0.07)'
          : 'inset 0 1px 0 rgba(255,255,255,0.05)',
      }}
    >
      <div className="pill__icon" style={{ color: theme.accent }}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3z" />
          <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
          <line x1="12" y1="19" x2="12" y2="22" />
          <line x1="9" y1="22" x2="15" y2="22" />
        </svg>
      </div>
      {style === 'bars'
        ? <BarsPreview levelsRef={levelsRef} accent={theme.accent} />
        : <WaveformCanvas style={style} width={42} height={20} levelsRef={levelsRef} accent={theme.accentRgb} idleMotion />}
    </div>
  )
}

function StyleSwatch({ s, active, onPick, levelsRef, theme }: {
  s: StyleDef
  active: boolean
  onPick: () => void
  levelsRef: React.RefObject<number[]>
  theme: PillThemeDef
}) {
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
      {/* The swatch runs the style itself, on the pill's own ground — the
          motion is what's being chosen, and it must read on that colour. */}
      <span
        className="grid h-6 w-13 shrink-0 place-items-center overflow-hidden rounded-(--r-xs) border border-(--border-soft)"
        style={{ background: theme.bg }}
      >
        {s.id === 'bars'
          ? <BarsPreview levelsRef={levelsRef} accent={theme.accent} />
          : <WaveformCanvas style={s.id} width={44} height={16} levelsRef={levelsRef} accent={theme.accentRgb} idleMotion />}
      </span>

      <span className="min-w-0 flex-1">
        <motion.span
          className="block truncate text-[12px] font-semibold tracking-[-0.01em]"
          initial={false}
          animate={{ color: active ? 'var(--accent)' : 'var(--fg)' }}
          transition={{ duration: 0.2 }}
        >
          {s.label}
        </motion.span>
        <span className="block truncate text-[10px] text-muted-foreground">{s.hint}</span>
      </span>

      {active && (
        <span className="grid size-4 shrink-0 place-items-center rounded-full bg-(--accent)">
          <Check size={9} strokeWidth={3.5} className="text-(--accent-fg)" />
        </span>
      )}
    </motion.button>
  )
}

/** How the pill draws your voice while it records. */
export const WaveformTab = memo(function WaveformTab() {
  const { waveformStyle, setWaveformStyle, pillTheme } = useAppStore()
  const levelsRef = usePreviewLevels()
  const current = WAVEFORM_STYLES.find((s) => s.id === waveformStyle) ?? WAVEFORM_STYLES[0]
  const theme = pillThemeDef(pillTheme)

  const handleSelect = (id: WaveformStyle) => {
    setWaveformStyle(id)
    void emit(EVENTS.PILL_WAVEFORM_STYLE_CHANGED, id)
  }

  return (
    <div className="flex gap-4 rounded-(--r-lg) border border-(--border-soft) bg-(--panel) p-4">
      <div className="flex shrink-0 flex-col gap-2.5">
        {/* Backdrop mixed from the pill's own ground, matching the theme
            preview above it, so the pill has something to sit off. */}
        <div
          className="grid h-21.5 w-55 place-items-center rounded-(--r-md) border border-(--border-soft) shadow-(--shadow-sm)"
          style={{
            background: theme.bg === '#ffffff'
              ? '#e8eaf0'
              : `color-mix(in srgb, ${theme.bg} 60%, #111)`,
          }}
        >
          <PreviewPill style={waveformStyle} levelsRef={levelsRef} theme={theme} />
        </div>
        <div className="flex items-baseline gap-2">
          <span className="text-[13px] font-bold tracking-[-0.02em] text-(--fg)">{current.label}</span>
          <span className="min-w-0 truncate text-[11px] text-muted-foreground">{current.hint}</span>
        </div>
      </div>

      <div className="flex min-w-0 flex-1 flex-col gap-1.5">
        <span className="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
          <AudioLines size={11} />
          Waveform
        </span>
        <div className="grid grid-cols-2 gap-1.5">
          {WAVEFORM_STYLES.map((s) => (
            <StyleSwatch
              key={s.id}
              s={s}
              active={waveformStyle === s.id}
              onPick={() => handleSelect(s.id)}
              levelsRef={levelsRef}
              theme={theme}
            />
          ))}
        </div>
      </div>
    </div>
  )
})
