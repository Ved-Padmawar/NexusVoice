import { useEffect, useState, useCallback, useRef, useMemo, useLayoutEffect } from 'react'
import { motion, AnimatePresence, useReducedMotion, useMotionValue, animate } from 'framer-motion'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Check, Pause, Play } from 'lucide-react'
import { EVENTS } from '../lib/events'
import { COMMANDS } from '../lib/commands'
import { extractErrorMessage } from '../lib/errors'
import type { ModelInfo } from '../types'
import type { PillTheme, WaveformStyle } from '../store/uiSlice'
import { WaveformCanvas } from '../components/WaveformCanvas'
import { pillThemeDef } from '../lib/pillThemes'
import { STORE_PERSIST_KEY } from '../store/persistKey'
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

// The pill element's own box; the window around it is sized by pill_geometry.rs.
const CARD_W = 332
const CARD_MIN_H = 84
const CARD_MAX_H = 186
const CARD_RADIUS = 28
const STRIP_H = 42
const TEXT_PAD = 27

const CAPSULE_W = 104
const CAPSULE_H = 32

const pillSpring = { type: 'spring' as const, stiffness: 380, damping: 30, mass: 0.8 }
const cardSpring = { type: 'spring' as const, stiffness: 210, damping: 32, mass: 1 }
const growSpring = { type: 'spring' as const, stiffness: 260, damping: 34, mass: 0.9 }
/** Critically damped — a bounce on close reads as the pill wobbling shut. */
const collapseSpring = { type: 'spring' as const, stiffness: 400, damping: 40, mass: 0.7 }

type PillState = 'idle' | 'recording' | 'dictation' | 'dictation-paused' | 'processing' | 'error' | 'downloading'

type LivePartial = { committed: string; tentative: string }

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

const SPINNER_COLORS: Record<PillTheme, { proc: [string, string]; dl: [string, string] }> = {
  steel:    { proc: ['rgba(148,168,200,0.15)', 'rgba(148,168,200,0.9)'], dl: ['rgba(245,158,11,0.15)', 'rgba(245,158,11,0.9)'] },
  midnight: { proc: ['rgba(26,209,209,0.15)',  'rgba(26,209,209,0.9)'],  dl: ['rgba(245,158,11,0.15)', 'rgba(245,158,11,0.9)'] },
  canvas:   { proc: ['rgba(58,91,217,0.15)',   'rgba(58,91,217,0.9)'],   dl: ['rgba(245,158,11,0.15)', 'rgba(245,158,11,0.9)'] },
  dawn:     { proc: ['rgba(228,56,0,0.15)',    'rgba(228,56,0,0.9)'],    dl: ['rgba(245,158,11,0.15)', 'rgba(245,158,11,0.9)'] },
}

function readPersisted<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(STORE_PERSIST_KEY)
    if (!raw) return fallback
    const parsed = JSON.parse(raw) as { state?: Record<string, unknown> }
    return (parsed?.state?.[key] as T) ?? fallback
  } catch {
    return fallback
  }
}

export function PillApp() {
  const [state, setState] = useState<PillState>('idle')
  const [errorMsg, setErrorMsg] = useState('')
  const [downloadPct, setDownloadPct] = useState(0)
  const modelReadyRef = useRef(false)
  const [tooltip, setTooltip] = useState('')
  const tooltipTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const barsRef = useRef<(HTMLSpanElement | null)[]>([])
  const [pillTheme, setPillTheme] = useState<PillTheme>(() => readPersisted<PillTheme>('pillTheme', 'steel'))
  const [waveformStyle, setWaveformStyle] = useState<WaveformStyle>(() => readPersisted<WaveformStyle>('waveformStyle', 'bars'))
  const [liveTranscript, setLiveTranscript] = useState(() => readPersisted<boolean>('liveTranscript', false))
  const [partial, setPartial] = useState<LivePartial | null>(null)
  const [roomy, setRoomy] = useState(false)
  const cardHeight = useMotionValue(CAPSULE_H)
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
    liveTranscript && (isRecording || isDictation) && partial !== null
  // Opening waits on the window being large enough, or the card is clipped for
  // the frames the resize IPC takes. Closing does not — the shrink trails it.
  const expanded = wantsCard && roomy

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

  // Waveform bars are driven by the Rust capture thread via `pill:waveform`
  // (8 spectrum levels, 0–1). The backend stops emitting and sends a zeroed
  // frame on stop, so the bars settle flat on their own.
  useEffect(() => {
    let cancelled = false
    let unlisten: (() => void) | undefined
    listen<number[]>(EVENTS.PILL_WAVEFORM, (e) => {
      if (cancelled) return
      const levels = e.payload
      if (levels.length !== FLAT_BARS.length) return
      const norm = levels.map((lvl) => Number.isFinite(lvl) ? Math.min(Math.max(lvl, 0), 1) : 0)
      levelsRef.current = norm
      // Keep the existing CSS height easing without rendering the whole pill
      // for every audio frame. Canvas styles consume the same levels ref.
      barsRef.current.forEach((bar, i) => updateBar(bar, norm[i]))
    }).then(fn => { if (!cancelled) unlisten = fn; else fn() })
    return () => { cancelled = true; unlisten?.() }
  }, [])

  useEffect(() => {
    let cancelled = false
    const unlisteners: (() => void)[] = []
    const setup = async () => {
      const u1 = await listen<WaveformStyle>(EVENTS.PILL_WAVEFORM_STYLE_CHANGED, (e) => {
        if (!cancelled) setWaveformStyle(e.payload)
      })
      if (cancelled) { u1(); return }
      unlisteners.push(u1)
      const u2 = await listen<PillTheme>(EVENTS.PILL_THEME_CHANGED, (e) => {
        if (!cancelled) setPillTheme(e.payload)
      })
      if (cancelled) { u2(); return }
      unlisteners.push(u2)
      const u3 = await listen<boolean>(EVENTS.PILL_LIVE_TRANSCRIPT_CHANGED, (e) => {
        if (!cancelled) setLiveTranscript(e.payload)
      })
      if (cancelled) { u3(); return }
      unlisteners.push(u3)
    }
    setup()
    return () => { cancelled = true; unlisteners.forEach(fn => fn()) }
  }, [])

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

  useEffect(() => () => {
    if (tooltipTimerRef.current) clearTimeout(tooltipTimerRef.current)
  }, [])

  // Check model status and listen for download events
  useEffect(() => {
    let cancelled = false
    const unlisteners: (() => void)[] = []

    // Skip if a download event already set the state — that one is fresher.
    invoke<ModelInfo>(COMMANDS.GET_MODEL_INFO)
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

    const setup = async () => {
      const um1 = await listen(EVENTS.MODEL_DOWNLOAD_START, () => {
        if (cancelled) return
        modelReadyRef.current = false
        setDownloadPct(0)
        setState(s => s === 'idle' ? 'downloading' : s)
      })
      if (cancelled) { um1(); return }
      unlisteners.push(um1)

      const um2 = await listen<{ pct: number }>(EVENTS.MODEL_DOWNLOAD_PROGRESS, (e) => {
        if (cancelled) return
        setDownloadPct(e.payload?.pct ?? 0)
        setState(s => s === 'idle' || s === 'downloading' ? 'downloading' : s)
      })
      if (cancelled) { um2(); return }
      unlisteners.push(um2)

      const um3 = await listen(EVENTS.MODEL_DOWNLOAD_COMPLETE, () => {
        if (cancelled) return
        modelReadyRef.current = true
        setState('idle')
      })
      if (cancelled) { um3(); return }
      unlisteners.push(um3)

      const um4 = await listen(EVENTS.MODEL_DOWNLOAD_ERROR, () => {
        if (cancelled) return
        setState('idle')
      })
      if (cancelled) { um4(); return }
      unlisteners.push(um4)

      const um5 = await listen(EVENTS.MODEL_DOWNLOAD_CANCELLED, () => {
        if (cancelled) return
        setState('idle')
        setDownloadPct(0)
      })
      if (cancelled) { um5(); return }
      unlisteners.push(um5)

      // Active model deleted — block recording until one is downloaded again.
      const um6 = await listen(EVENTS.MODEL_EVICTED, () => {
        if (cancelled) return
        modelReadyRef.current = false
        setState(s => (s === 'recording' || s === 'dictation') ? s : 'idle')
      })
      if (cancelled) { um6(); return }
      unlisteners.push(um6)

      // Deleted active model but another is on disk — that one is ready.
      const um7 = await listen(EVENTS.MODEL_SWITCHED, () => {
        if (cancelled) return
        modelReadyRef.current = true
      })
      if (cancelled) { um7(); return }
      unlisteners.push(um7)
    }

    setup()
    return () => {
      cancelled = true
      unlisteners.forEach(fn => fn())
    }
  }, [])

  useEffect(() => {
    let cancelled = false
    const unlisteners: (() => void)[] = []
    const setup = async () => {
      const u1 = await listen<LivePartial>(EVENTS.TRANSCRIPTION_PARTIAL, (e) => {
        if (cancelled) return
        const p = e.payload
        if (!p || (!p.committed && !p.tentative)) return
        setPartial(p)
      })
      if (cancelled) { u1(); return }
      unlisteners.push(u1)
      const u2 = await listen(EVENTS.TRANSCRIPTION_PARTIAL_END, () => {
        if (!cancelled) setPartial(null)
      })
      if (cancelled) { u2(); return }
      unlisteners.push(u2)
    }
    setup()
    return () => { cancelled = true; unlisteners.forEach(fn => fn()) }
  }, [])

  useEffect(() => {
    let cancelled = false
    const unlisteners: (() => void)[] = []

    const setup = async () => {
      const u1 = await listen(EVENTS.HOTKEY_PRESSED, async () => {
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
          await serialize(() => invoke(COMMANDS.START_TRANSCRIPTION))
        } catch (err: unknown) {
          const raw = extractErrorMessage(err, String(err))
          const msg = raw.toLowerCase().includes('no input device') || raw.toLowerCase().includes('no microphone')
            ? 'No microphone found'
            : raw.toLowerCase().includes('permission') || raw.toLowerCase().includes('access denied')
              ? 'Mic access denied'
              : raw
          setErrorMsg(msg)
          setState('error')
          isRecordingRef.current = false
          setTimeout(() => setState('idle'), 3000)
        }
      })
      if (cancelled) { u1(); return }
      unlisteners.push(u1)

      const u2 = await listen(EVENTS.HOTKEY_RELEASED, async () => {
        if (!isRecordingRef.current) return
        setPartial(null)
        setState('processing')
        try {
          await serialize(() => invoke(COMMANDS.STOP_TRANSCRIPTION))
        } catch (err: unknown) {
          const msg = extractErrorMessage(err, String(err))
          setErrorMsg(msg)
          setState('error')
          setTimeout(() => setState('idle'), 3000)
        } finally {
          isRecordingRef.current = false
        }
      })
      if (cancelled) { u2(); return }
      unlisteners.push(u2)

      const u3 = await listen<string>(EVENTS.TRANSCRIPTION_COMPLETE, async (event) => {
        const text = event.payload
        if (text) {
          try {
            await invoke(COMMANDS.TYPE_TEXT, { text })
          } catch { /* clipboard briefly locked — text is still on clipboard, user can paste */ }
        }
        setPartial(null)
        setState('idle')
        isRecordingRef.current = false
        isDictationRef.current = false
      })
      if (cancelled) { u3(); return }
      unlisteners.push(u3)

      const u4 = await listen<string>(EVENTS.TRANSCRIPTION_ERROR, (event) => {
        setErrorMsg(event.payload ?? 'Transcription failed')
        setPartial(null)
        setState('error')
        isRecordingRef.current = false
        isDictationRef.current = false
        setTimeout(() => setState('idle'), 3000)
      })
      if (cancelled) { u4(); return }
      unlisteners.push(u4)

      const u5 = await listen(EVENTS.DICTATION_HOTKEY_PRESSED, async () => {
        if (isRecordingRef.current || stateRef.current === 'processing' || stateRef.current === 'downloading') return
        if (!modelReadyRef.current && !isDictationRef.current) {
          showTooltip('No model installed — download one in Settings')
          return
        }

        try {
          if (!isDictationRef.current) {
            isDictationRef.current = true
            setState('dictation')
            await invoke(COMMANDS.START_DICTATION)
            return
          }

          if (stateRef.current === 'dictation') {
            await invoke(COMMANDS.PAUSE_DICTATION)
            setState('dictation-paused')
            return
          }

          if (stateRef.current === 'dictation-paused') {
            await invoke(COMMANDS.RESUME_DICTATION)
            setState('dictation')
          }
        } catch (err: unknown) {
          const msg = extractErrorMessage(err, String(err))
          setErrorMsg(msg)
          setState('error')
          isDictationRef.current = false
          setTimeout(() => setState('idle'), 3000)
        }
      })
      if (cancelled) { u5(); return }
      unlisteners.push(u5)

      const u6 = await listen(EVENTS.DICTATION_COMMIT_HOTKEY_PRESSED, async () => {
        if (!isDictationRef.current || isRecordingRef.current || stateRef.current === 'processing') return
        setPartial(null)
        setState('processing')
        try {
          await invoke(COMMANDS.COMMIT_DICTATION)
        } catch (err: unknown) {
          const msg = extractErrorMessage(err, String(err))
          setErrorMsg(msg)
          setState('error')
          isDictationRef.current = false
          setTimeout(() => setState('idle'), 3000)
        }
      })
      if (cancelled) { u6(); return }
      unlisteners.push(u6)
    }

    setup()
    return () => {
      cancelled = true
      unlisteners.forEach(fn => fn())
    }
  }, [showTooltip, serialize])

  // Resize the native window once on opening and once after closing settles.
  // Skipped until actually grown, so a pill that never expands is never moved.
  const grownRef = useRef(false)
  useEffect(() => {
    let cancelled = false
    if (wantsCard) {
      grownRef.current = true
      invoke(COMMANDS.RESIZE_PILL, { expanded: true })
        .then(() => { if (!cancelled) setRoomy(true) })
        .catch(() => {})
      return () => { cancelled = true }
    }
    if (!grownRef.current) return
    grownRef.current = false
    // The window follows only once the box has settled; resizing mid-spring tears.
    const seq = animate(cardHeight, CAPSULE_H, reduceMotion ? { duration: 0 } : collapseSpring)
    seq.then(() => {
      if (cancelled) return
      setRoomy(false)
      void invoke(COMMANDS.RESIZE_PILL, { expanded: false }).catch(() => {})
    }).catch(() => {})
    return () => { cancelled = true; seq.stop() }
  }, [wantsCard, cardHeight, reduceMotion])

  // Measure the inner span, not the scroller — scrollHeight clamps at the
  // ceiling, stalling growth. Height rides a MotionValue so partials don't
  // re-render.
  useLayoutEffect(() => {
    if (!expanded) return
    // A not-yet-painted span measures 0, which the clamp turns into the ceiling.
    const inner = innerRef.current?.offsetHeight ?? 0
    if (inner === 0) return
    const want = Math.min(CARD_MAX_H, Math.max(CARD_MIN_H, STRIP_H + inner + TEXT_PAD))
    animate(cardHeight, want, reduceMotion ? { duration: 0 } : growSpring)
    const el = textRef.current
    if (el) el.scrollTop = el.scrollHeight
  }, [expanded, partial, cardHeight, reduceMotion])

  const handleToggleDictationPause = useCallback(async () => {
    try {
      if (state === 'dictation') {
        await invoke(COMMANDS.PAUSE_DICTATION)
        setState('dictation-paused')
      } else if (state === 'dictation-paused') {
        await invoke(COMMANDS.RESUME_DICTATION)
        setState('dictation')
      }
    } catch (err: unknown) {
      setErrorMsg(extractErrorMessage(err, String(err)))
      setState('error')
      isDictationRef.current = false
      setTimeout(() => setState('idle'), 3000)
    }
  }, [state])

  const handleCommitDictation = useCallback(async () => {
    if (!isDictationRef.current) return
    setPartial(null)
    setState('processing')
    try {
      await invoke(COMMANDS.COMMIT_DICTATION)
    } catch (err: unknown) {
      setErrorMsg(extractErrorMessage(err, String(err)))
      setState('error')
      isDictationRef.current = false
      setTimeout(() => setState('idle'), 3000)
    }
  }, [])

  const theme = useMemo(() => pillThemeDef(pillTheme), [pillTheme])

  /** A canvas is sized in pixels, so it is keyed on its slot — remounting at
   *  the final width rather than stretching from the old one. */
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
              key={`${waveformStyle}-${width}`}
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
      {tooltip && <div className="pill-tooltip">{tooltip}</div>}

      <motion.div
        // Keep the outgoing transcript in its card layout until the close
        // finishes. Switching to a flex row during its fade rewraps the text.
        className={`pill pill--${state}${roomy ? ' pill--expanded' : ''}${roomy && !expanded ? ' pill--closing' : ''}`}
        data-pill-theme={pillTheme}
        initial={false}
        animate={{
          width: expanded ? CARD_W : (PILL_WIDTH[state] ?? CAPSULE_W),
          borderRadius: expanded ? CARD_RADIUS : CAPSULE_H / 2,
        }}
        transition={
          reduceMotion ? { duration: 0 }
          : expanded ? cardSpring
          // Closing from a card — must match the height's spring, or width and
          // height shut on different curves.
          : roomy ? collapseSpring
          : pillSpring
        }
        style={{ height: cardHeight, overflow: 'hidden' }}
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
                transition: reduceMotion ? { duration: 0 } : { height: growSpring, opacity: { duration: 0.3 } },
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
                <span className="pill__committed">{partial?.committed}</span>
                {partial?.tentative && <> <span className="pill__tentative">{partial.tentative}</span></>}
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
          isDictation && !roomy ? (
            <div className="pill__dictation" aria-label={isPaused ? 'Dictation paused' : 'Dictation recording'}>
              {stripContent}
            </div>
          ) : (
            <div className="pill__strip">{stripContent}</div>
          )
        )}

        {/* Preserve the transcript's bottom inset while its strip fades out. */}
        {roomy && !showStrip && <div className="pill__strip" aria-hidden="true" />}

        {state === 'processing' && <Spinner colors={SPINNER_COLORS[pillTheme].proc} />}

        {state === 'downloading' && (
          <>
            <Spinner colors={SPINNER_COLORS[pillTheme].dl} />
            <span className="pill__pct">{downloadPct}%</span>
          </>
        )}
      </motion.div>
    </div>
  )
}
