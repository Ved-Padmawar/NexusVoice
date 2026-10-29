import { memo, useEffect, useState, useCallback, useRef, useMemo, useSyncExternalStore } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { CheckIcon, PauseIcon, PlayIcon } from '@phosphor-icons/react'
import { commands, type PillSettings, type TranscriptionPartial } from '../shared/bindings'
import { extractErrorMessage } from '../shared/errors'
import { useEventListener } from '../shared/hooks'
import { subscribePillSettings } from './pillSettings'
import { subscribeRecordingState } from './recordingState'
import { WaveformCanvas } from '../shared/ui/WaveformCanvas'
import { pillThemeDef } from './pillThemes'
import { CARD_W, usePillGeometry } from './usePillGeometry'
import './PillApp.css'

const PILL_WIDTH: Record<string, number> = {
  idle: 104,
  recording: 80,
  dictation: 104,
  'dictation-paused': 104,
  processing: 32,
  downloading: 32,
  error: 104,
}

type PillState = 'idle' | 'recording' | 'dictation' | 'dictation-paused' | 'processing' | 'error' | 'downloading'

// Partials bypass React state so only <LiveText> re-renders per frame.
// Clearing keeps the last text so the card's exit fade still shows it.
let livePartial: TranscriptionPartial | null = null
let hasLivePartial = false
const partialSubs = new Set<() => void>()
const setPartial = (p: TranscriptionPartial | null) => {
  if (p) livePartial = p
  hasLivePartial = p !== null
  partialSubs.forEach((fn) => fn())
}
const subscribePartial = (fn: () => void) => {
  partialSubs.add(fn)
  return () => { partialSubs.delete(fn) }
}
const getPartial = () => livePartial
const getHasPartial = () => hasLivePartial

const LiveText = memo(function LiveText() {
  const partial = useSyncExternalStore(subscribePartial, getPartial)
  return (
    <>
      <span className="pill__committed">{partial?.committed}</span>
      {partial?.tentative && <> <span className="pill__tentative">{partial.tentative}</span></>}
    </>
  )
})

const MIN_H = 3
const MAX_H = 16
const FLAT_BARS = [3, 3, 3, 3, 3, 3, 3, 3]

function updateBar(bar: HTMLSpanElement | null, level: number) {
  if (!bar) return
  const height = `${Math.round(MIN_H + (MAX_H - MIN_H) * level)}px`
  if (bar.style.height !== height) bar.style.height = height
}

function Spinner({ colors }: { colors: [string, string] }) {
  return (
    <div className="pill__spinner-slot">
      <div
        className="pill__spinner"
        style={{ borderRadius: '50%', border: `1.5px solid ${colors[0]}`, borderTopColor: colors[1] }}
      />
    </div>
  )
}

const DOWNLOAD_SPINNER: [string, string] = ['rgba(245,158,11,0.15)', 'rgba(245,158,11,0.9)']

function processingSpinner(accentRgb: string): [string, string] {
  return [`rgba(${accentRgb},0.15)`, `rgba(${accentRgb},0.9)`]
}

const DEFAULT_SETTINGS: PillSettings = { theme: 'steel', waveformStyle: 'bars', liveTranscript: false, startSound: false }

/** Map a capture failure to the short label the pill has room for. */
function captureErrorLabel(message: string): string {
  const raw = message.toLowerCase()
  if (raw.includes('no input device') || raw.includes('no microphone')) return 'No microphone found'
  if (raw.includes('permission') || raw.includes('access denied')) return 'Mic access denied'
  return message
}

export function PillApp() {
  const [state, setState] = useState<PillState>('idle')
  const [errorMsg, setErrorMsg] = useState('')
  const [downloadPct, setDownloadPct] = useState(0)
  const [tooltip, setTooltip] = useState('')
  const tooltipTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const errorTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const barsRef = useRef<(HTMLSpanElement | null)[]>([])
  const [settings, setSettings] = useState<PillSettings>(DEFAULT_SETTINGS)
  const { theme: pillTheme, waveformStyle, liveTranscript } = settings
  const hasPartial = useSyncExternalStore(subscribePartial, getHasPartial)
  // Canvas styles read levels off a ref so a 30 Hz stream never re-renders.
  const levelsRef = useRef<number[]>(new Array(FLAT_BARS.length).fill(0))
  const textRef = useRef<HTMLDivElement | null>(null)
  const innerRef = useRef<HTMLSpanElement | null>(null)
  const pillRef = useRef<HTMLDivElement | null>(null)

  const isRecording = state === 'recording'
  const isDictation = state === 'dictation' || state === 'dictation-paused'
  const isPaused = state === 'dictation-paused'

  const wantsCard =
    liveTranscript && (isRecording || isDictation) && hasPartial
  const { roomy, expanded, width, height, radius, headroom } = usePillGeometry(
    wantsCard, tooltip !== '', PILL_WIDTH[state], pillRef, innerRef, textRef,
  )

  const handleDragStart = useCallback((e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest('button')) return
    e.preventDefault()
    // startDragging must fire synchronously on the mousedown tick — awaiting
    // it breaks drag on Windows.
    void getCurrentWindow().startDragging()
  }, [])

  // Waveform levels come from the capture thread, 8 bands of 0–1. The backend
  // sends a zeroed frame on stop, so the bars settle flat on their own.
  useEventListener('pillWaveform', (levels) => {
    const norm = levels.map((lvl) => Math.min(Math.max(lvl, 0), 1))
    levelsRef.current = norm
    // Keep the existing CSS height easing without rendering the whole pill
    // for every audio frame. Canvas styles consume the same levels ref.
    barsRef.current.forEach((bar, i) => updateBar(bar, norm[i]))
  })

  useEffect(() => subscribePillSettings(setSettings), [])

  useEffect(() => {
    void getCurrentWindow().setAlwaysOnTop(true)
  }, [])

  // Windows passes clicks through everywhere but this box.
  useEffect(() => {
    const pill = pillRef.current
    if (!pill) return
    const report = () => {
      const { x, y, width, height } = pill.getBoundingClientRect()
      commands.setPillHitbox({ x, y, width, height }).catch(() => {})
    }
    report()
    const observer = new ResizeObserver(report)
    observer.observe(pill)
    window.addEventListener('resize', report)
    return () => {
      observer.disconnect()
      window.removeEventListener('resize', report)
    }
  }, [])

  const showTooltip = useCallback((msg: string) => {
    setTooltip(msg)
    if (tooltipTimerRef.current) clearTimeout(tooltipTimerRef.current)
    tooltipTimerRef.current = setTimeout(() => setTooltip(''), 3000)
  }, [])

  // Resets only if still showing the error — a recording may have started since.
  const showError = useCallback((msg: string) => {
    setErrorMsg(msg)
    setState('error')
    if (errorTimerRef.current) clearTimeout(errorTimerRef.current)
    errorTimerRef.current = setTimeout(() => {
      setState(current => current === 'error' ? 'idle' : current)
    }, 3000)
  }, [])

  useEffect(() => () => {
    if (tooltipTimerRef.current) clearTimeout(tooltipTimerRef.current)
    if (errorTimerRef.current) clearTimeout(errorTimerRef.current)
  }, [])

  // A download event may land first; it is fresher, so it wins.
  useEffect(() => {
    let cancelled = false
    commands.getModelInfo()
      .then(info => {
        if (cancelled) return
        setState(current => {
          if (current === 'downloading') return current
          if (info.downloading && current === 'idle') {
            return 'downloading'
          }
          return current
        })
      })
      .catch(() => {})
    return () => { cancelled = true }
  }, [])

  useEventListener('modelDownloadStart', () => {
    setDownloadPct(0)
    setState(s => s === 'idle' ? 'downloading' : s)
  })
  useEventListener('modelDownloadProgress', ({ pct }) => {
    setDownloadPct(pct)
    setState(s => s === 'idle' || s === 'downloading' ? 'downloading' : s)
  })
  useEventListener('modelDownloadComplete', () => {
    setState(current => current === 'downloading' ? 'idle' : current)
  })
  useEventListener('modelDownloadError', () => setState(current => current === 'downloading' ? 'idle' : current))
  useEventListener('modelDownloadCancelled', () => {
    setState(current => current === 'downloading' ? 'idle' : current)
    setDownloadPct(0)
  })
  useEventListener('transcriptionPartial', (p) => {
    if (p.committed || p.tentative) setPartial(p)
  })
  useEventListener('transcriptionPartialEnd', () => setPartial(null))
  useEffect(() => () => { livePartial = null; setPartial(null) }, [])

  useEffect(() => subscribeRecordingState(snapshot => {
    const next = snapshot.status
    if (next !== 'recording' && next !== 'dictation' && next !== 'dictation-paused') setPartial(null)
    setState(current => next === 'idle' && (current === 'error' || current === 'downloading') ? current : next)
  }), [])

  useEventListener('transcriptionComplete', ({ pasteError, saved }) => {
    if (pasteError) showTooltip(saved ? "Couldn't paste — it's saved in your history" : "Couldn't paste or save this dictation")
  })
  useEventListener('transcriptionError', message => showError(captureErrorLabel(message)))
  useEventListener('transcriptionWarning', showTooltip)

  const handleCommitDictation = useCallback(async () => {
    try {
      await commands.commitDictation()
    } catch (err: unknown) {
      showTooltip(extractErrorMessage(err, 'Could not finish dictation'))
    }
  }, [showTooltip])

  const handleToggleDictationPause = useCallback(async () => {
    try {
      if (state === 'dictation') await commands.pauseDictation()
      else if (state === 'dictation-paused') await commands.resumeDictation()
    } catch (err: unknown) {
      showTooltip(extractErrorMessage(err, 'Could not change dictation'))
    }
  }, [state, showTooltip])

  const theme = useMemo(() => pillThemeDef(pillTheme), [pillTheme])

  /** Keep the canvas alive when its slot changes so its history and peaks
   *  survive the capsule/card transition. */
  const renderWaveform = (dictation = false) => {
    const width = roomy ? 54 : (dictation ? 45 : 42)
    return (
      <div className={`pill__waveform pill__wave-slot${dictation ? ' pill__waveform--dictation' : ''}`}>
        {waveformStyle === 'bars'
          ? FLAT_BARS.map((_, i) => (
              <span key={i} className="pill__bar" ref={bar => {
                barsRef.current[i] = bar
                updateBar(bar, levelsRef.current[i])
              }} />
            ))
          : (
            <WaveformCanvas
              style={waveformStyle}
              width={width}
              height={20}
              levelsRef={levelsRef}
              accent={theme.accentRgb}
            />
          )}
      </div>
    )
  }

  const showIcon = state === 'idle' || state === 'recording' || state === 'error'

  const stripContent = (
    <>
      {showIcon && (
        <div className="pill__icon">
          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M12 2a3 3 0 0 1 3 3v7a3 3 0 0 1-6 0V5a3 3 0 0 1 3-3z"/>
            <path d="M19 10a7 7 0 0 1-14 0"/>
            <line x1="12" y1="19" x2="12" y2="22"/>
            <line x1="9" y1="22" x2="15" y2="22"/>
          </svg>
        </div>
      )}

      {state === 'idle' && <span className="pill__brand">NexusVoice</span>}

      {isRecording && (
        <>
          {renderWaveform()}
          <span className="pill__hint">Hold to speak</span>
        </>
      )}

      {isDictation && (
        <>
          <button
            type="button"
            className="pill__control"
            onMouseDown={(e) => e.stopPropagation()}
            onClick={handleToggleDictationPause}
            aria-label={isPaused ? 'Resume dictation' : 'Pause dictation'}
            title={isPaused ? 'Resume' : 'Pause'}
          >
            {isPaused ? <PlayIcon size={11} weight="bold" /> : <PauseIcon size={11} weight="bold" />}
          </button>
          {renderWaveform(true)}
          <button
            type="button"
            className="pill__control pill__control--commit"
            onMouseDown={(e) => e.stopPropagation()}
            onClick={handleCommitDictation}
            aria-label="Commit dictation"
            title="Save"
          >
            <CheckIcon size={12} weight="bold" />
          </button>
        </>
      )}

      {state === 'error' && <span className="pill__error-label" title={errorMsg}>Error</span>}
    </>
  )

  const showStrip = state !== 'processing' && state !== 'downloading'

  return (
    <div style={{ position: 'relative', display: 'flex', alignItems: 'flex-end', justifyContent: 'center' }}>
      {/* Shown once the window has room; over a card, inside its top edge. */}
      {tooltip && headroom && <div className={`pill-tooltip${roomy ? ' pill-tooltip--inset' : ''}`}>{tooltip}</div>}

      <div
        // Keep the outgoing transcript in its card layout until the close
        // finishes. Switching to a flex row during its fade rewraps the text.
        className={`pill pill--${state}${roomy ? ' pill--expanded' : ''}${roomy && !expanded ? ' pill--closing' : ''}`}
        data-pill-theme={pillTheme}
        ref={pillRef}
        style={{ width, height, borderRadius: radius, overflow: 'hidden' }}
        onMouseDown={handleDragStart}
        role="status"
        aria-label={`NexusVoice: ${state}`}
      >
        {roomy && (
          <>
            <div
              className="pill__tide"
              style={{ background: `linear-gradient(to top, rgba(${theme.accentRgb},0.16), transparent)` }}
            />
            <div
              ref={textRef}
              className="pill__transcript"
              style={{ '--card-w': `${CARD_W}px` } as React.CSSProperties}
            >
              <span ref={innerRef} style={{ display: 'block' }}>
                <LiveText />
                {!isPaused && (
                  <span className="pill__caret" />
                )}
              </span>
            </div>
          </>
        )}

        {showStrip && (
          <div className={isDictation && !roomy ? 'pill__dictation' : `pill__strip${roomy && !expanded && state === 'idle' ? ' pill__strip--returning' : ''}`}
            aria-label={isDictation ? (isPaused ? 'Dictation paused' : 'Dictation recording') : undefined}>
            {stripContent}
          </div>
        )}

        {/* Preserve the transcript's bottom inset while its strip fades out. */}
        {roomy && !showStrip && <div className="pill__strip" aria-hidden="true" />}

        {state === 'processing' && <Spinner colors={processingSpinner(theme.accentRgb)} />}

        {state === 'downloading' && (
          <>
            <Spinner colors={DOWNLOAD_SPINNER} />
            <span className="pill__pct">{downloadPct}%</span>
          </>
        )}
      </div>
    </div>
  )
}
