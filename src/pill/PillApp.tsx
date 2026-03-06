import { useEffect, useState, useCallback, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import './PillApp.css'

type PillState = 'idle' | 'recording' | 'processing' | 'error'

export function PillApp() {
  const [state, setState] = useState<PillState>('idle')
  const [errorMsg, setErrorMsg] = useState('')
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

  // Real mic amplitude → bar heights
  useEffect(() => {
    if (state !== 'recording') {
      // Cleanup
      if (rafRef.current) { cancelAnimationFrame(rafRef.current); rafRef.current = null }
      if (audioCtxRef.current) { audioCtxRef.current.close(); audioCtxRef.current = null }
      if (streamRef.current) { streamRef.current.getTracks().forEach(t => t.stop()); streamRef.current = null }
      setTimeout(() => setBarHeights([3, 3, 3, 3, 3, 3, 3, 3]), 0)
      return
    }

    const BAR_COUNT = 8
    const MIN_H = 3
    const MAX_H = 16
    const weights = [0.5, 0.7, 0.85, 1.0, 1.0, 0.85, 0.7, 0.5]

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
        analyser.getByteFrequencyData(data)
        // RMS across lower frequency bins (voice range)
        const bins = data.slice(0, 32)
        const rms = Math.sqrt(bins.reduce((s, v) => s + v * v, 0) / bins.length)
        const norm = Math.min(rms / 80, 1) // 0..1

        const heights = Array.from({ length: BAR_COUNT }, (_, i) => {
          const h = MIN_H + (MAX_H - MIN_H) * norm * weights[i]
          return Math.max(MIN_H, Math.round(h))
        })
        setBarHeights(heights)
        rafRef.current = requestAnimationFrame(tick)
      }
      rafRef.current = requestAnimationFrame(tick)
    }).catch(() => {
      // No mic permission — fall back to idle heights
    })

    return () => {
      if (rafRef.current) { cancelAnimationFrame(rafRef.current); rafRef.current = null }
      if (audioCtxRef.current) { audioCtxRef.current.close(); audioCtxRef.current = null }
      if (streamRef.current) { streamRef.current.getTracks().forEach(t => t.stop()); streamRef.current = null }
    }
  }, [state])

  // Ensure pill stays above the Windows taskbar at runtime
  useEffect(() => {
    void getCurrentWindow().setAlwaysOnTop(true)
  }, [])

  const isRecordingRef = useRef(false)

  useEffect(() => {
    const unlisteners: (() => void)[] = []

    const setup = async () => {
      const u1 = await listen('hotkey-pressed', async () => {
        // Guard against key-repeat: if already recording, ignore subsequent presses
        if (isRecordingRef.current) return
        isRecordingRef.current = true
        setState('recording')
        try {
          await invoke('start_transcription')
        } catch (err: unknown) {
          const msg = err && typeof err === 'object' && 'message' in err ? (err as { message: string }).message : String(err)
          console.error('Failed to start:', msg)
          setErrorMsg(msg)
          setState('error')
          isRecordingRef.current = false
          setTimeout(() => setState('idle'), 3000)
        }
      })
      unlisteners.push(u1)

      const u2 = await listen('hotkey-released', async () => {
        if (!isRecordingRef.current) return
        isRecordingRef.current = false
        setState('processing')
        try {
          await invoke('stop_transcription')
        } catch (err: unknown) {
          const msg = err && typeof err === 'object' && 'message' in err ? (err as { message: string }).message : String(err)
          console.error('Failed to stop:', msg)
          setErrorMsg(msg)
          setState('error')
          setTimeout(() => setState('idle'), 3000)
        }
      })
      unlisteners.push(u2)

      const u3 = await listen<string>('transcription-complete', async (event) => {
        const text = event.payload
        if (text) {
          await invoke('type_text', { text })
        }
        setState('idle')
      })
      unlisteners.push(u3)

      const u4 = await listen<string>('transcription-error', (event) => {
        console.error('Transcription error:', event.payload)
        setErrorMsg(event.payload ?? 'Transcription failed')
        setState('error')
        setTimeout(() => setState('idle'), 3000)
      })
      unlisteners.push(u4)

    }

    setup()
    return () => {
      unlisteners.forEach(fn => fn())
    }
  }, [])

  return (
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
  )
}
