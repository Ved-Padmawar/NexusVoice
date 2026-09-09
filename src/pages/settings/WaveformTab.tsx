import { useShallow } from 'zustand/react/shallow'
import { memo, useEffect, useRef } from 'react'
import { emit } from '@tauri-apps/api/event'
import { Check, AudioWaveform } from 'lucide-react'
import { useAppStore } from '../../store/useAppStore'
import { EVENTS } from '../../lib/events'
import { WaveformCanvas } from '../../components/WaveformCanvas'
import { BARS } from '../../lib/waveform'
import { pillThemeDef, type PillThemeDef } from '../../lib/pillThemes'
import { Section } from '../../components/Section'
import type { WaveformStyle } from '../../store/uiSlice'

type StyleDef = {
  id: WaveformStyle
  label: string
  hint: string
}

const WAVEFORM_STYLES: StyleDef[] = [
  { id: 'bars',     label: 'Bars',     hint: 'Live level meter' },
  { id: 'memo',     label: 'Memo',     hint: 'Scrolling voice note' },
  { id: 'eq',       label: 'EQ',       hint: 'Retro equalizer' },
  { id: 'spectrum', label: 'Spectrum', hint: 'Analyser columns' },
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

/** The pill at its real recording width, in the user's pill theme. */
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
        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M12 2a3 3 0 0 1 3 3v7a3 3 0 0 1-6 0V5a3 3 0 0 1 3-3z" />
          <path d="M19 10a7 7 0 0 1-14 0" />
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

function StyleCard({ s, active, onPick, levelsRef, theme }: {
  s: StyleDef
  active: boolean
  onPick: () => void
  levelsRef: React.RefObject<number[]>
  theme: PillThemeDef
}) {
  return (
    <button
      type="button"
      onClick={onPick}
      aria-pressed={active}
      title={`${s.label} — ${s.hint}`}
      className="pick flex flex-col gap-2 p-2"
    >
      {/* The real pill, running the style. The backdrop is mixed from the
          pill's own ground so it has something to float off, the way it does
          over another window. */}
      <span
        className="grid h-14 w-full place-items-center rounded-(--r-md)"
        style={{
          background: theme.bg === '#ffffff'
            ? '#e8eaf0'
            : `color-mix(in srgb, ${theme.bg} 55%, #101215)`,
        }}
      >
        <PreviewPill style={s.id} levelsRef={levelsRef} theme={theme} />
      </span>
      <span className="flex w-full items-center gap-1 px-0.5">
        <span className={`min-w-0 flex-1 truncate text-left text-[11.5px] font-semibold ${active ? 'text-(--on-soft)' : 'text-(--fg)'}`}>
          {s.label}
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

/** How the pill draws your voice while it records. */
export const WaveformTab = memo(function WaveformTab() {
  const { waveformStyle, setWaveformStyle, pillTheme } = useAppStore(useShallow(s => ({
    waveformStyle: s.waveformStyle,
    setWaveformStyle: s.setWaveformStyle,
    pillTheme: s.pillTheme,
  })))
  const levelsRef = usePreviewLevels()
  const theme = pillThemeDef(pillTheme)

  const handleSelect = (id: WaveformStyle) => {
    setWaveformStyle(id)
    void emit(EVENTS.PILL_WAVEFORM_STYLE_CHANGED, id)
  }

  return (
    <Section
      title="Waveform"
      Icon={AudioWaveform}
      description="How the pill draws your voice. Each card is live."
      bodyClassName="grid grid-cols-4 gap-2.5 p-3.5"
    >
      {WAVEFORM_STYLES.map((s) => (
        <StyleCard
          key={s.id}
          s={s}
          active={waveformStyle === s.id}
          onPick={() => handleSelect(s.id)}
          levelsRef={levelsRef}
          theme={theme}
        />
      ))}
    </Section>
  )
})
