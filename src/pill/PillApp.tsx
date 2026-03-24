import { useEffect, useState, useCallback, useRef } from 'react'
import { motion } from 'framer-motion'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { EVENTS } from '../lib/events'
import { COMMANDS } from '../lib/commands'
import { extractErrorMessage } from '../lib/errors'
import type { ModelInfo } from '../types'
import './PillApp.css'

const PILL_WIDTH: Record<string, number> = {
  idle: 104,
  recording: 104,
  processing: 32,
  downloading: 32,
  error: 104,
}

const pillSpring = { type: 'spring' as const, stiffness: 380, damping: 30, mass: 0.8 }

type PillState = 'idle' | 'recording' | 'processing' | 'error' | 'downloading'

const BAR_COUNT = 8
const MIN_H = 3
const MAX_H = 16
const WEIGHTS = [0.5, 0.7, 0.85, 1.0, 1.0, 0.85, 0.7, 0.5]
const ANALYSER_FFT_SIZE = 256
const FREQ_BIN_COUNT = 32

export function PillApp() {
  const [state, setState] = useState<PillState>('idle')
  const [errorMsg, setErrorMsg] = useState('')
  const [downloadPct, setDownloadPct] = useState(0)
  const modelReadyRef = useRef(false)
  const [tooltip, setTooltip] = useState('')
  const tooltipTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const [barHeights, setBarHeights] = useState([3, 3, 3, 3, 3, 3, 3, 3])
  const audioCtxRef = useRef<AudioContext | null>(null)
  const analyserRef = useRef<AnalyserNode | null>(null)
  const rafRef = useRef<number | null>(null)
  const streamRef = useRef<MediaStream | null>(null)

  // Start dragging the window on mousedown
  const handleDragStart = useCallback((e: React.MouseEvent) => {
    // Only drag from the pill body, not buttons
    if ((e.target as HTMLElement).closest('button')) return
    e.preventDefault()
    // startDragging must be called without await so it fires synchronously
    // on the same mousedown event tick — awaiting it breaks drag on Windows
    void getCurrentWindow().startDragging()
  }, [])

  const startAudio = useCallback(() => {
    if (audioCtxRef.current) return // already running
    navigator.mediaDevices.getUserMedia({ audio: true, video: false }).then(stream => {
      streamRef.current = stream
      const ctx = new AudioContext()
      audioCtxRef.current = ctx
      const source = ctx.createMediaStreamSource(stream)
      const analyser = ctx.createAnalyser()
      analyser.fftSize = ANALYSER_FFT_SIZE
      analyser.smoothingTimeConstant = 0.6
      source.connect(analyser)
      analyserRef.current = analyser

      const data = new Uint8Array(analyser.frequencyBinCount)
      const tick = () => {
        if (!analyserRef.current) return
        analyserRef.current.getByteFrequencyData(data)
        const bins = data.slice(0, FREQ_BIN_COUNT)
        const rms = Math.sqrt(bins.reduce((s, v) => s + v * v, 0) / bins.length)
        const norm = Math.min(rms / 80, 1)
        const heights = Array.from({ length: BAR_COUNT }, (_, i) => {
          const h = MIN_H + (MAX_H - MIN_H) * norm * WEIGHTS[i]
          return Math.max(MIN_H, Math.round(h))
        })
        setBarHeights(heights)
        rafRef.current = requestAnimationFrame(tick)
      }

      rafRef.current = requestAnimationFrame(tick)
    }).catch(() => { /* no mic permission */ })
  }, [])

  const stopAudio = useCallback(() => {
    if (rafRef.current) { cancelAnimationFrame(rafRef.current); rafRef.current = null }
    if (audioCtxRef.current) { audioCtxRef.current.close(); audioCtxRef.current = null }
    if (streamRef.current) { streamRef.current.getTracks().forEach(t => t.stop()); streamRef.current = null }
    analyserRef.current = null
    requestAnimationFrame(() => setBarHeights([3, 3, 3, 3, 3, 3, 3, 3]))
  }, [])

  // Cleanup on unmount
  useEffect(() => {
    return () => { stopAudio() }
  }, [stopAudio])

  // Start/stop audio based on recording state
  const isRecording = state === 'recording'
  useEffect(() => {
    if (isRecording) {
      startAudio()
    } else {
      stopAudio()
    }
  }, [isRecording, startAudio, stopAudio])

  // Ensure pill stays above the Windows taskbar at runtime
  useEffect(() => {
    void getCurrentWindow().setAlwaysOnTop(true)
  }, [])

  const isRecordingRef = useRef(false)

  const showTooltip = useCallback((msg: string) => {
    setTooltip(msg)
    if (tooltipTimerRef.current) clearTimeout(tooltipTimerRef.current)
    tooltipTimerRef.current = setTimeout(() => setTooltip(''), 3000)
  }, [])


  // Check model status and listen for download events
  useEffect(() => {
    let cancelled = false
    const unlisteners: (() => void)[] = []

    // Fire model info fetch independently — don't block listener registration
    invoke<ModelInfo>(COMMANDS.GET_MODEL_INFO)
      .then(info => {
        if (cancelled) return
        if (info.downloaded) {
          modelReadyRef.current = true
        } else if (info.downloading) {
          modelReadyRef.current = false
          setDownloadPct(info.downloadProgress ?? 0)
          setState('downloading')
        }
      })
      .catch(() => { /* ignore */ })

    const setup = async () => {
      // Events for ongoing progress updates
      const um1 = await listen(EVENTS.MODEL_DOWNLOAD_START, () => {
        if (cancelled) return
        modelReadyRef.current = false
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
        if (isRecordingRef.current) return
        // Block recording if model not ready
        if (!modelReadyRef.current) {
          showTooltip('Model downloading… please wait')
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
          await invoke(COMMANDS.TYPE_TEXT, { text })
        }
        setState('idle')
      })
      if (cancelled) { u3(); return }
      unlisteners.push(u3)

      const u4 = await listen<string>(EVENTS.TRANSCRIPTION_ERROR, (event) => {
        setErrorMsg(event.payload ?? 'Transcription failed')
        setState('error')
        setTimeout(() => setState('idle'), 3000)
      })
      if (cancelled) { u4(); return }
      unlisteners.push(u4)
    }

    setup()
    return () => {
      cancelled = true
      unlisteners.forEach(fn => fn())
    }
  }, [showTooltip])

  return (
    <div style={{ position: 'relative', width: '100%', height: '100%' }}>
      {/* Tooltip bubble — shown when recording blocked */}
      {tooltip && (
        <div className="pill-tooltip">{tooltip}</div>
      )}

      <motion.div
        className={`pill pill--${state}`}
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
            <svg width="11" height="11" viewBox="0 0 24 22" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
              <rect x="9" y="2" width="6" height="12" rx="3" />
              <path d="M5 10a7 7 0 0 0 14 0" />
              <line x1="12" y1="19" x2="12" y2="22" />
              <line x1="9" y1="22" x2="15" y2="22" />
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
        {state === 'error' && (
          <span className="pill__error-label" title={errorMsg}>Error</span>
        )}

        {state === 'downloading' && (
          <span className="pill__pct">{downloadPct}%</span>
        )}
      </motion.div>
    </div>
  )
}
