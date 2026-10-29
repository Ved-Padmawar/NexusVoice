import { useShallow } from 'zustand/react/shallow'
import { memo, useEffect, useRef } from 'react'
import { useAppStore } from '../../app/store/useAppStore'
import { Swatch } from '../../shared/ui/Swatch'
import { WaveformCanvas } from '../../shared/ui/WaveformCanvas'
import { BARS, ease } from '../../shared/waveform'
import { subscribeWhileVisible } from '../../shared/frameClock'
import { pillThemeDef, type PillThemeDef } from '../../pill/pillThemes'
import type { WaveformStyle } from '../../shared/types'

type StyleDef = {
  id: WaveformStyle
  label: string
  hint: string
}

const WAVEFORM_STYLES: StyleDef[] = [
  { id: 'bars',  label: 'Bars',  hint: 'Live level meter' },
  { id: 'memo',  label: 'Memo',  hint: 'Scrolling voice note' },
  { id: 'eq',    label: 'EQ',    hint: 'Retro equalizer' },
  { id: 'spectrum', label: 'Spectrum', hint: 'Analyser columns' },
]

const MIN_H = 3
const MAX_H = 16

/**
 * Speech-shaped levels for the preview. The real frames come from the capture
 * thread, which is silent here — random noise would make every style look
 * alike, so this fakes syllables, phonemes and breaths instead.
 */
function usePreviewLevels(rootRef: React.RefObject<HTMLElement | null>) {
  const levelsRef = useRef<number[]>(new Array(BARS).fill(0))

  useEffect(() => {
    const smooth = new Array(BARS).fill(0)
    let t = 0
    let phraseEnd = 2.4
    let breathing = false
    let phone = 0
    let phoneEnd = 0
    let vowel = true

    const advance = (dt: number) => {
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
        smooth[i] += (target[i] - smooth[i]) * ease(rising ? 0.55 : 0.12 + i * 0.008, dt)
      }
      levelsRef.current = smooth.slice()
    }

    const root = rootRef.current
    return root ? subscribeWhileVisible(root, advance) : undefined
  }, [rootRef])

  return levelsRef
}

/** The `bars` style is DOM-drawn, so the preview mirrors that rather than
 *  reimplementing it on canvas — what you see is what the pill renders. */
function BarsPreview({ levelsRef, accent }: { levelsRef: React.RefObject<number[]>; accent: string }) {
  const wrapRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    const wrap = wrapRef.current
    if (!wrap) return

    let drawn: number[] | null = null
    return subscribeWhileVisible(wrap, () => {
      // Same array as last frame means the generator is paused.
      if (levelsRef.current === drawn) return
      const levels = levelsRef.current
      drawn = levels
      const spans = wrap.children
      for (let i = 0; i < spans.length; i++) {
        const v = levels[i] ?? 0
        ;(spans[i] as HTMLElement).style.height =
          `${Math.max(MIN_H, Math.round(MIN_H + (MAX_H - MIN_H) * v))}px`
      }
    })
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
        flexShrink: 0,
        background: theme.bg,
        backgroundOrigin: 'border-box',
        borderColor: theme.border,
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
        : <WaveformCanvas style={style} width={42} height={20} levelsRef={levelsRef} accent={theme.accentRgb} idleMotion preview />}
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
    <Swatch
      active={active}
      onPick={onPick}
      // The real recording pill running this style, so the motion being
      // chosen is seen on the colour it will be drawn on.
      preview={<PreviewPill style={s.id} levelsRef={levelsRef} theme={theme} />}
      label={s.label}
      hint={s.hint}
    />
  )
}

/** How the pill draws your voice while it records. */
export const WaveformTab = memo(function WaveformTab() {
  const { waveformStyle, pillTheme, updatePill } = useAppStore(useShallow(s => ({
    waveformStyle: s.pill.waveformStyle,
    pillTheme: s.pill.theme,
    updatePill: s.updatePill,
  })))
  const rootRef = useRef<HTMLDivElement | null>(null)
  const levelsRef = usePreviewLevels(rootRef)
  const theme = pillThemeDef(pillTheme)

  const handleSelect = (id: WaveformStyle) => { void updatePill({ waveformStyle: id }) }

  return (
    <section>
    <h2 className="mb-2 text-[13px] font-semibold text-fg-2">Waveform</h2>
    <div ref={rootRef} className="flex gap-4 rounded-lg border border-line-soft bg-panel p-4">
      <div className="flex shrink-0 flex-col gap-2.5">
        <div
          className="grid h-21.5 w-55 place-items-center rounded-md border border-line-soft shadow-(--shadow-sm)"
          style={{ background: theme.stage }}
        >
          <PreviewPill style={waveformStyle} levelsRef={levelsRef} theme={theme} />
        </div>
        <span className="text-[11px] text-muted">How the pill draws your voice</span>
      </div>

      <div className="grid min-w-0 flex-1 grid-cols-2 content-center gap-1.5">
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
    </section>
  )
})
