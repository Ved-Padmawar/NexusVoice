import { useEffect, useState, useCallback, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import './PillApp.css'

type PillState = 'idle' | 'recording' | 'processing' | 'error' | 'downloading'

const BAR_COUNT = 8
const MIN_H = 3
const MAX_H = 16
const WEIGHTS = [0.5, 0.7, 0.85, 1.0, 1.0, 0.85, 0.7, 0.5]

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
      analyser.fftSize = 256
      analyser.smoothingTimeConstant = 0.6
      source.connect(analyser)
      analyserRef.current = analyser

      const data = new Uint8Array(analyser.frequencyBinCount)
      const tick = () => {
        if (!analyserRef.current) return
        analyserRef.current.getByteFrequencyData(data)
        const bins = data.slice(0, 32)
        const rms = Math.sqrt(bins.reduce((s, v) => s + v * v, 0) / bins.length)
        const norm = Math.min(rms / 80, 1)
        const heights = Array.from({ length: BAR_COUNT }, (_, i) => {
          const h = MIN_H + (MAX_H - MIN_H) * norm * WEIGHTS[i]
          return Math.max(MIN_H, Math.round(h))
        })
        setBarHeights(heights)
        rafRef.current = requestAnimationFrame(tick)
      }

      ;(analyserRef as React.MutableRefObject<AnalyserNode & { _tick?: () => void }>).current._tick = tick
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
    invoke<{ downloaded: boolean; downloading: boolean; downloadProgress: number; downloadError: string | null }>('get_model_info')
      .then(info => {
        if (cancelled) return
        if (info.downloaded) {
          modelReadyRef.current = true
        } else if (info.downloading) {
          modelReadyRef.current = false
          setState('downloading')
          setDownloadPct(info.downloadProgress)
        }
      })
      .catch(() => { /* ignore */ })

    const setup = async () => {
      // Events for ongoing progress updates
      const um1 = await listen('model-download-start', () => {
        if (cancelled) return
        modelReadyRef.current = false
        setState(s => s === 'idle' ? 'downloading' : s)
        setDownloadPct(0)
      })
      unlisteners.push(um1)

      const um2 = await listen<number>('model-download-progress', (e) => {
        if (cancelled) return
        setDownloadPct(e.payload)
        setState(s => s === 'idle' || s === 'downloading' ? 'downloading' : s)
      })
      unlisteners.push(um2)

      const um3 = await listen('model-download-complete', () => {
        if (cancelled) return
        modelReadyRef.current = true
        setDownloadPct(100)
        setState('idle')
      })
      unlisteners.push(um3)

      const um4 = await listen<string>('model-download-error', () => {
        if (cancelled) return
        setState('idle')
      })
      unlisteners.push(um4)
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
      const u1 = await listen('hotkey-pressed', async () => {
        if (isRecordingRef.current) return
        // Block recording if model not ready
        if (!modelReadyRef.current) {
          showTooltip('Model downloading… please wait')
          return
        }
        isRecordingRef.current = true
        setState('recording')
        try {
          await invoke('start_transcription')
        } catch (err: unknown) {
          const msg = err && typeof err === 'object' && 'message' in err ? (err as { message: string }).message : String(err)
          setErrorMsg(msg)
          setState('error')
          isRecordingRef.current = false
          setTimeout(() => setState('idle'), 3000)
        }
      })
      if (cancelled) { u1(); return }
      unlisteners.push(u1)

      const u2 = await listen('hotkey-released', async () => {
        if (!isRecordingRef.current) return
        isRecordingRef.current = false
        setState('processing')
        try {
          await invoke('stop_transcription')
        } catch (err: unknown) {
          const msg = err && typeof err === 'object' && 'message' in err ? (err as { message: string }).message : String(err)
          setErrorMsg(msg)
          setState('error')
          setTimeout(() => setState('idle'), 3000)
        }
      })
      if (cancelled) { u2(); return }
      unlisteners.push(u2)

      const u3 = await listen<string>('transcription-complete', async (event) => {
        const text = event.payload
        if (text) {
          await invoke('type_text', { text })
        }
        setState('idle')
      })
      if (cancelled) { u3(); return }
      unlisteners.push(u3)

      const u4 = await listen<string>('transcription-error', (event) => {
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

      <div
        className={`pill pill--${state}`}
        onMouseDown={handleDragStart}
        role="status"
        aria-label={`NexusVoice: ${state}`}
      >
        {/* Lightning bolt icon — always visible */}
        <div className="pill__icon">
          <svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor">
            <path d="M13 2L4.5 13.5H11L10 22L19.5 10.5H13L13 2Z" />
          </svg>
        </div>

        {/* Center content */}
        <div className="pill__center">
          {state === 'idle' && (
            <span className="pill__brand">NexusVoice</span>
          )}
          {state === 'downloading' && (
            <div className="pill__processing">
              <div className="pill__spinner" />
              <span className="pill__proc-label">{downloadPct}%</span>
            </div>
          )}
          {state === 'recording' && (
            <div className="pill__waveform">
              {barHeights.map((h, i) => (
                <span key={i} className="pill__bar" style={{ height: `${h}px` }} />
              ))}
            </div>
          )}
          {state === 'processing' && (
            <div className="pill__processing">
              <div className="pill__spinner" />
            </div>
          )}
          {state === 'error' && (
            <span className="pill__error-label" title={errorMsg}>Error</span>
          )}
        </div>
      </div>
    </div>
  )
}
