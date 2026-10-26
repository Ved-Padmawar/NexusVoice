import { memo, useEffect, useState, useCallback, useRef, useMemo, useSyncExternalStore } from 'react'
import { motion, AnimatePresence, useReducedMotion } from 'framer-motion'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Check, Pause, Play } from 'lucide-react'
import { commands, type PillSettings, type TranscriptionPartial } from '../bindings'
import { extractErrorMessage } from '../lib/errors'
import { useEventListener } from '../lib/hooks'
import { subscribePillSettings } from '../lib/pillSettings'
import { WaveformCanvas } from '../components/WaveformCanvas'
import { pillThemeDef } from '../lib/pillThemes'
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
      <motion.div
        className="pill__spinner"
        style={{ borderRadius: '50%', border: `1.5px solid ${colors[0]}`, borderTopColor: colors[1] }}
        animate={{ rotate: 360 }}
        transition={{ duration: 1, ease: 'linear', repeat: Infinity }}
      />
    </div>
  )
}

const DOWNLOAD_SPINNER: [string, string] = ['rgba(245,158,11,0.15)', 'rgba(245,158,11,0.9)']

function processingSpinner(accentRgb: string): [string, string] {
  return [`rgba(${accentRgb},0.15)`, `rgba(${accentRgb},0.9)`]
}

const DEFAULT_SETTINGS: PillSettings = { theme: 'steel', waveformStyle: 'bars', liveTranscript: false }

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
  const modelReadyRef = useRef(false)
  const [tooltip, setTooltip] = useState('')
  const tooltipTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const errorTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const barsRef = useRef<(HTMLSpanElement | null)[]>([])
  const [settings, setSettings] = useState<PillSettings>(DEFAULT_SETTINGS)
  const { theme: pillTheme, waveformStyle, liveTranscript } = settings
  const hasPartial = useSyncExternalStore(subscribePartial, getHasPartial)
  // Canvas styles read levels off a ref so a 30 Hz stream never re-renders.
  const levelsRef = useRef<number[]>(new Array(FLAT_BARS.length).fill(0))
  const stateRef = useRef<PillState>('idle')
  const textRef = useRef<HTMLDivElement | null>(null)
  const innerRef = useRef<HTMLSpanElement | null>(null)
  const reduceMotion = useReducedMotion()

  const isRecording = state === 'recording'
  const isDictation = state === 'dictation' || state === 'dictation-paused'
  const isPaused = state === 'dictation-paused'

  const wantsCard =
    liveTranscript && (isRecording || isDictation) && hasPartial
  const { roomy, expanded, width, height, radius, headroom } = usePillGeometry(
    wantsCard, tooltip !== '', PILL_WIDTH[state], reduceMotion, innerRef, textRef,
  )

  useEffect(() => {
    stateRef.current = state
  }, [state])

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

  const isRecordingRef = useRef(false)
  const isDictationRef = useRef(false)
  // Serializes start/stop so a fast press-and-release can't invoke
  // stop_transcription while start_transcription is still awaiting.
  const inFlightRef = useRef<Promise<unknown>>(Promise.resolve())

  /** Queue `fn` behind any in-flight start/stop and await the result. */
  const serialize = useCallback(<T,>(fn: () => Promise<T>): Promise<T> => {
    const next = inFlightRef.current.then(fn, fn)
    // Keep the chain alive when a link rejects; callers handle their own errors.
    inFlightRef.current = next.catch(() => {})
    return next
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
      if (stateRef.current === 'error') setState('idle')
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
          if (info.downloading) {
            modelReadyRef.current = false
            return 'downloading'
          }
          if (info.downloaded) modelReadyRef.current = true
          return current
        })
      })
      .catch(() => {})
    return () => { cancelled = true }
  }, [])

  useEventListener('modelDownloadStart', () => {
    modelReadyRef.current = false
    setDownloadPct(0)
    setState(s => s === 'idle' ? 'downloading' : s)
  })
  useEventListener('modelDownloadProgress', ({ pct }) => {
    setDownloadPct(pct)
    setState(s => s === 'idle' || s === 'downloading' ? 'downloading' : s)
  })
  useEventListener('modelDownloadComplete', () => {
    modelReadyRef.current = true
    setState('idle')
  })
  useEventListener('modelDownloadError', () => setState('idle'))
  useEventListener('modelDownloadCancelled', () => {
    setState('idle')
    setDownloadPct(0)
  })
  // Active model deleted: block recording until one is downloaded again.
  useEventListener('modelEvicted', () => {
    modelReadyRef.current = false
    setState(s => (s === 'recording' || s === 'dictation') ? s : 'idle')
  })
  // Deleted the active model but another is on disk: that one is ready.
  useEventListener('modelSwitched', () => { modelReadyRef.current = true })

  useEventListener('transcriptionPartial', (p) => {
    if (p.committed || p.tentative) setPartial(p)
  })
  useEventListener('transcriptionPartialEnd', () => setPartial(null))
  useEffect(() => () => { livePartial = null; setPartial(null) }, [])

  const resetRecording = useCallback(() => {
    setPartial(null)
    isRecordingRef.current = false
    isDictationRef.current = false
  }, [])

  useEventListener('hotkeyPressed', async () => {
    if (isRecordingRef.current || isDictationRef.current) return
    // A finalize is still running — starting now would race it.
    if (stateRef.current === 'processing') return
    if (!modelReadyRef.current) {
      showTooltip(stateRef.current === 'downloading' ? 'Model downloading… please wait' : 'No model installed — download one in Settings')
      return
    }
    isRecordingRef.current = true
    setState('recording')
    try {
      await serialize(() => commands.startTranscription())
    } catch (err: unknown) {
      showError(captureErrorLabel(extractErrorMessage(err, String(err))))
      isRecordingRef.current = false
    }
  })

  useEventListener('hotkeyReleased', async () => {
    if (!isRecordingRef.current) return
    setPartial(null)
    setState('processing')
    try {
      await serialize(() => commands.stopTranscription())
    } catch (err: unknown) {
      showError(extractErrorMessage(err, String(err)))
    } finally {
      isRecordingRef.current = false
    }
  })

  // The backend saves and pastes the text itself; this only resets the pill.
  useEventListener('transcriptionComplete', ({ pasteError }) => {
    resetRecording()
    setState('idle')
    if (pasteError) showTooltip("Couldn't paste — it's saved in your history")
  })

  useEventListener('transcriptionError', (message) => {
    resetRecording()
    showError(message)
  })

  useEventListener('recordingCancelled', () => {
    resetRecording()
    setState('idle')
  })

  // Not an error: the recording carries on and keeps what it captured.
  useEventListener('transcriptionWarning', showTooltip)

  useEventListener('dictationHotkeyPressed', async () => {
    if (isRecordingRef.current || stateRef.current === 'processing' || stateRef.current === 'downloading') return
    if (!modelReadyRef.current && !isDictationRef.current) {
      showTooltip('No model installed — download one in Settings')
      return
    }

    try {
      if (!isDictationRef.current) {
        isDictationRef.current = true
        setState('dictation')
        await commands.startDictation()
        return
      }

      if (stateRef.current === 'dictation') {
        await commands.pauseDictation()
        setState('dictation-paused')
        return
      }

      if (stateRef.current === 'dictation-paused') {
        await commands.resumeDictation()
        setState('dictation')
      }
    } catch (err: unknown) {
      showError(extractErrorMessage(err, String(err)))
      isDictationRef.current = false
    }
  })

  const handleCommitDictation = useCallback(async () => {
    if (!isDictationRef.current || isRecordingRef.current || stateRef.current === 'processing') return
    setPartial(null)
    setState('processing')
    try {
      await commands.commitDictation()
    } catch (err: unknown) {
      showError(extractErrorMessage(err, String(err)))
      isDictationRef.current = false
    }
  }, [showError])

  useEventListener('dictationCommitHotkeyPressed', handleCommitDictation)

  const handleToggleDictationPause = useCallback(async () => {
    try {
      if (state === 'dictation') {
        await commands.pauseDictation()
        setState('dictation-paused')
      } else if (state === 'dictation-paused') {
        await commands.resumeDictation()
        setState('dictation')
      }
    } catch (err: unknown) {
      showError(extractErrorMessage(err, String(err)))
      isDictationRef.current = false
    }
  }, [state, showError])

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
            {isPaused ? <Play size={11} strokeWidth={2.4} /> : <Pause size={11} strokeWidth={2.4} />}
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
            <Check size={12} strokeWidth={2.5} />
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

      <motion.div
        // Keep the outgoing transcript in its card layout until the close
        // finishes. Switching to a flex row during its fade rewraps the text.
        className={`pill pill--${state}${roomy ? ' pill--expanded' : ''}${roomy && !expanded ? ' pill--closing' : ''}`}
        data-pill-theme={pillTheme}
        initial={false}
        style={{ width, height, borderRadius: radius, overflow: 'hidden' }}
        onMouseDown={handleDragStart}
        role="status"
        aria-label={`NexusVoice: ${state}`}
      >
        <AnimatePresence>
          {expanded && (
            <motion.div
              key="tide"
              className="pill__tide"
              initial={{ height: 0, opacity: 0 }}
              animate={{
                height: 88,
                opacity: isPaused ? 0.4 : 1,
                transition: reduceMotion ? { duration: 0 } : { height: { duration: 0.28, ease: [0.22, 1, 0.36, 1] }, opacity: { duration: 0.2 } },
              }}
              exit={{ opacity: 0, transition: { duration: reduceMotion ? 0 : 0.09 } }}
              style={{ background: `linear-gradient(to top, rgba(${theme.accentRgb},0.16), transparent)` }}
            />
          )}
        </AnimatePresence>

        <AnimatePresence>
          {expanded && (
            <motion.div
              key="transcript"
              ref={textRef}
              className="pill__transcript"
              style={{ '--card-w': `${CARD_W}px` } as React.CSSProperties}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1, transition: reduceMotion ? { duration: 0 } : { duration: 0.24, delay: 0.14 } }}
              exit={{ opacity: 0, transition: { duration: reduceMotion ? 0 : 0.09 } }}
            >
              <span ref={innerRef} style={{ display: 'block' }}>
                <LiveText />
                {!isPaused && (
                  <motion.span
                    className="pill__caret"
                    animate={{ opacity: [1, 1, 0.15, 0.15] }}
                    transition={{ duration: 1.05, times: [0, 0.55, 0.5501, 1], repeat: Infinity, ease: 'linear' }}
                  />
                )}
              </span>
            </motion.div>
          )}
        </AnimatePresence>

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
      </motion.div>
    </div>
  )
}
