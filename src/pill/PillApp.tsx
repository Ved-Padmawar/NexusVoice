import { useEffect, useState, useCallback, useRef } from 'react'
import { motion } from 'framer-motion'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Check, Pause, Play } from 'lucide-react'
import { EVENTS } from '../lib/events'
import { COMMANDS } from '../lib/commands'
import { extractErrorMessage } from '../lib/errors'
import type { ModelInfo } from '../types'
import type { PillTheme } from '../store/uiSlice'
import { STORE_PERSIST_KEY } from '../store/useAppStore'
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

const pillSpring = { type: 'spring' as const, stiffness: 380, damping: 30, mass: 0.8 }

type PillState = 'idle' | 'recording' | 'dictation' | 'dictation-paused' | 'processing' | 'error' | 'downloading'

const MIN_H = 3
const MAX_H = 16
const FLAT_BARS = [3, 3, 3, 3, 3, 3, 3, 3]

const SPINNER_COLORS: Record<PillTheme, { proc: [string, string]; dl: [string, string] }> = {
  dark:  { proc: ['rgba(120,162,244,0.15)', 'rgba(120,162,244,0.9)'],  dl: ['rgba(245,158,11,0.15)', 'rgba(251,191,36,0.9)'] },
  steel: { proc: ['rgba(148,168,200,0.15)', 'rgba(148,168,200,0.9)'],  dl: ['rgba(245,158,11,0.15)', 'rgba(245,158,11,0.9)'] },
  light: { proc: ['rgba(58,91,217,0.15)',   'rgba(58,91,217,0.9)'],    dl: ['rgba(245,158,11,0.15)', 'rgba(245,158,11,0.9)'] },
  teal:  { proc: ['rgba(91,184,196,0.15)',  'rgba(91,184,196,0.9)'],   dl: ['rgba(245,158,11,0.15)', 'rgba(245,158,11,0.9)'] },
}

function readPillTheme(): PillTheme {
  try {
    const raw = localStorage.getItem(STORE_PERSIST_KEY)
    if (!raw) return 'dark'
    const parsed = JSON.parse(raw) as { state?: { pillTheme?: PillTheme } }
    return parsed?.state?.pillTheme ?? 'dark'
  } catch {
    return 'dark'
  }
}

export function PillApp() {
  const [state, setState] = useState<PillState>('idle')
  const [errorMsg, setErrorMsg] = useState('')
  const [downloadPct, setDownloadPct] = useState(0)
  const modelReadyRef = useRef(false)
  const [tooltip, setTooltip] = useState('')
  const tooltipTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const [barHeights, setBarHeights] = useState(FLAT_BARS)
  const [pillTheme, setPillTheme] = useState<PillTheme>(readPillTheme)
  const stateRef = useRef<PillState>('idle')

  useEffect(() => {
    stateRef.current = state
  }, [state])

  // Start dragging the window on mousedown
  const handleDragStart = useCallback((e: React.MouseEvent) => {
    // Only drag from the pill body, not buttons
    if ((e.target as HTMLElement).closest('button')) return
    e.preventDefault()
    // startDragging must be called without await so it fires synchronously
    // on the same mousedown event tick — awaiting it breaks drag on Windows
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
      setBarHeights(levels.map((lvl) => {
        const norm = Math.min(Math.max(lvl, 0), 1)
        return Math.max(MIN_H, Math.round(MIN_H + (MAX_H - MIN_H) * norm))
      }))
    }).then(fn => { if (!cancelled) unlisten = fn; else fn() })
    return () => { cancelled = true; unlisten?.() }
  }, [])

  // Ensure pill stays above the Windows taskbar at runtime
  useEffect(() => {
    void getCurrentWindow().setAlwaysOnTop(true)
  }, [])

  // Sync pill theme from main window via event
  useEffect(() => {
    let cancelled = false
    let unlisten: (() => void) | undefined
    listen<PillTheme>(EVENTS.PILL_THEME_CHANGED, (e) => {
      if (!cancelled) setPillTheme(e.payload)
    }).then(fn => { if (!cancelled) unlisten = fn; else fn() })
    return () => { cancelled = true; unlisten?.() }
  }, [])

  const isRecordingRef = useRef(false)
  const isDictationRef = useRef(false)

  const showTooltip = useCallback((msg: string) => {
    setTooltip(msg)
    if (tooltipTimerRef.current) clearTimeout(tooltipTimerRef.current)
    tooltipTimerRef.current = setTimeout(() => setTooltip(''), 3000)
  }, [])


  // Check model status and listen for download events
  useEffect(() => {
    let cancelled = false
    const unlisteners: (() => void)[] = []

    // Fire model info fetch independently — don't block listener registration.
    // Only apply the result if no download event has already set the state,
    // to avoid racing with in-flight progress events.
    invoke<ModelInfo>(COMMANDS.GET_MODEL_INFO)
      .then(info => {
        if (cancelled) return
        setState(current => {
          if (current === 'downloading') return current // already driven by events
          if (info.downloading) {
            modelReadyRef.current = false
            setDownloadPct(info.downloadProgress ?? 0)
            return 'downloading'
          }
          if (info.downloaded) {
            modelReadyRef.current = true
          }
          return current
        })
      })
      .catch(() => { /* ignore */ })

    const setup = async () => {
      // Events for ongoing progress updates
      const um1 = await listen(EVENTS.MODEL_DOWNLOAD_START, () => {
        if (cancelled) return
        modelReadyRef.current = false
        setDownloadPct(0)
        setState(s => s === 'idle' ? 'downloading' : s)
      })
      unlisteners.push(um1)

      const um2 = await listen<number>(EVENTS.MODEL_DOWNLOAD_PROGRESS, (e) => {
        if (cancelled) return
        setDownloadPct(e.payload ?? 0)
        setState(s => s === 'idle' || s === 'downloading' ? 'downloading' : s)
      })
      unlisteners.push(um2)

      const um3 = await listen(EVENTS.MODEL_DOWNLOAD_COMPLETE, () => {
        if (cancelled) return
        modelReadyRef.current = true
        setState('idle')
      })
      unlisteners.push(um3)

      const um4 = await listen(EVENTS.MODEL_DOWNLOAD_ERROR, () => {
        if (cancelled) return
        setState('idle')
      })
      unlisteners.push(um4)

      const um5 = await listen(EVENTS.MODEL_DOWNLOAD_CANCELLED, () => {
        if (cancelled) return
        setState('idle')
        setDownloadPct(0)
      })
      unlisteners.push(um5)

      // Active model deleted — block recording until one is downloaded again.
      const um6 = await listen(EVENTS.MODEL_EVICTED, () => {
        if (cancelled) return
        modelReadyRef.current = false
        setState(s => (s === 'recording' || s === 'dictation') ? s : 'idle')
      })
      unlisteners.push(um6)

      // Deleted active model but another is on disk — that one is ready.
      const um7 = await listen(EVENTS.MODEL_SWITCHED, () => {
        if (cancelled) return
        modelReadyRef.current = true
      })
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
      const u1 = await listen(EVENTS.HOTKEY_PRESSED, async () => {
        if (isRecordingRef.current || isDictationRef.current) return
        // Block recording if model not ready
        if (!modelReadyRef.current) {
          showTooltip(stateRef.current === 'downloading' ? 'Model downloading… please wait' : 'No model installed — download one in Settings')
          return
        }
        isRecordingRef.current = true
        setState('recording')
        try {
          await invoke(COMMANDS.START_TRANSCRIPTION)
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
        setState('processing')
        try {
          await invoke(COMMANDS.STOP_TRANSCRIPTION)
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
        setState('idle')
        isRecordingRef.current = false
        isDictationRef.current = false
      })
      if (cancelled) { u3(); return }
      unlisteners.push(u3)

      const u4 = await listen<string>(EVENTS.TRANSCRIPTION_ERROR, (event) => {
        setErrorMsg(event.payload ?? 'Transcription failed')
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
  }, [showTooltip])

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

  return (
    <div style={{ position: 'relative', width: '100%', height: '100%' }}>
      {/* Tooltip bubble — shown when recording blocked */}
      {tooltip && (
        <div className="pill-tooltip">{tooltip}</div>
      )}

      <motion.div
        className={`pill pill--${state}`}
        data-pill-theme={pillTheme === 'dark' ? undefined : pillTheme}
        initial={{ width: 104 }}
        animate={{ width: PILL_WIDTH[state] ?? 104 }}

        transition={pillSpring}
        style={{ overflow: 'hidden' }}
        onMouseDown={handleDragStart}
        role="status"
        aria-label={`NexusVoice: ${state}`}
      >
        {/* Icon — only shown when pill is full width */}
        {(state === 'idle' || state === 'recording' || state === 'error') && (
          <div className="pill__icon">
            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M12 2a3 3 0 0 1 3 3v7a3 3 0 0 1-6 0V5a3 3 0 0 1 3-3z"/>
              <path d="M19 10a7 7 0 0 1-14 0"/>
              <line x1="12" y1="19" x2="12" y2="22"/>
              <line x1="9" y1="22" x2="15" y2="22"/>
            </svg>
          </div>
        )}

        {state === 'idle' && (
          <span className="pill__brand">NexusVoice</span>
        )}
        {state === 'recording' && (
          <div className="pill__waveform">
            {barHeights.map((h, i) => (
              <span key={i} className="pill__bar" style={{ height: `${h}px` }} />
            ))}
          </div>
        )}
        {(state === 'dictation' || state === 'dictation-paused') && (
          <div className="pill__dictation" aria-label={state === 'dictation-paused' ? 'Dictation paused' : 'Dictation recording'}>
            <button
              type="button"
              className="pill__control"
              onMouseDown={(e) => e.stopPropagation()}
              onClick={handleToggleDictationPause}
              aria-label={state === 'dictation-paused' ? 'Resume dictation' : 'Pause dictation'}
              title={state === 'dictation-paused' ? 'Resume' : 'Pause'}
            >
              {state === 'dictation-paused' ? <Play size={11} strokeWidth={2.4} /> : <Pause size={11} strokeWidth={2.4} />}
            </button>
            <div className="pill__waveform pill__waveform--dictation">
              {barHeights.map((h, i) => (
                <span key={i} className="pill__bar" style={{ height: `${h}px` }} />
              ))}
            </div>
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
          </div>
        )}
        {state === 'error' && (
          <span className="pill__error-label" title={errorMsg}>Error</span>
        )}

        {state === 'processing' && (
          <motion.div
            style={{
              position: 'absolute',
              inset: 4,
              borderRadius: '50%',
              border: `1.5px solid ${SPINNER_COLORS[pillTheme].proc[0]}`,
              borderTopColor: SPINNER_COLORS[pillTheme].proc[1],
            }}
            animate={{ rotate: 360 }}
            transition={{ duration: 1, ease: 'linear', repeat: Infinity }}
          />
        )}

        {state === 'downloading' && (
          <>
            <motion.div
              style={{
                position: 'absolute',
                inset: 4,
                borderRadius: '50%',
                border: `1.5px solid ${SPINNER_COLORS[pillTheme].dl[0]}`,
                borderTopColor: SPINNER_COLORS[pillTheme].dl[1],
              }}
              animate={{ rotate: 360 }}
              transition={{ duration: 1, ease: 'linear', repeat: Infinity }}
            />
            <span className="pill__pct">{downloadPct}%</span>
          </>
        )}
      </motion.div>
    </div>
  )
}
