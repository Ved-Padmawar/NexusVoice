import { useEffect, useState, useCallback, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import './PillApp.css'

type PillState = 'idle' | 'recording' | 'processing' | 'error' | 'downloading'

export function PillApp() {
  const [state, setState] = useState<PillState>('idle')
  const [elapsed, setElapsed] = useState(0)
  const [errorMsg, setErrorMsg] = useState('')
  const [downloadPct, setDownloadPct] = useState(0)
  const timerRef = useRef<number | null>(null)
  const startTimeRef = useRef<number>(0)

  // Start dragging the window on mousedown
  const handleDragStart = useCallback((e: React.MouseEvent) => {
    // Only drag from the pill body, not buttons
    if ((e.target as HTMLElement).closest('button')) return
    e.preventDefault()
    // startDragging must be called without await so it fires synchronously
    // on the same mousedown event tick — awaiting it breaks drag on Windows
    void getCurrentWindow().startDragging()
  }, [])

  // Timer for recording duration — derived from start time to avoid sync setState in effect
  useEffect(() => {
    if (state !== 'recording') return
    startTimeRef.current = Date.now()
    timerRef.current = window.setInterval(() => {
      setElapsed(Math.floor((Date.now() - startTimeRef.current) / 1000))
    }, 1000)
    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current)
        timerRef.current = null
      }
    }
  }, [state])

  // Format seconds to mm:ss
  const formatTime = (seconds: number) => {
    const m = Math.floor(seconds / 60).toString().padStart(2, '0')
    const s = (seconds % 60).toString().padStart(2, '0')
    return `${m}:${s}`
  }

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

      const u5 = await listen('model-download-start', () => {
        setState('downloading')
        setDownloadPct(0)
      })
      unlisteners.push(u5)

      const u6 = await listen<number>('model-download-progress', (event) => {
        setDownloadPct(event.payload)
      })
      unlisteners.push(u6)

      const u7 = await listen('model-download-complete', () => {
        setState('processing')
        setDownloadPct(0)
      })
      unlisteners.push(u7)
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
          <div className="pill__waveform-wrap">
            <div className="pill__waveform">
              {Array.from({ length: 5 }).map((_, i) => (
                <span key={i} className="pill__bar" />
              ))}
            </div>
            <span className="pill__timer">{formatTime(elapsed)}</span>
          </div>
        )}
        {state === 'processing' && (
          <div className="pill__processing">
            <div className="pill__spinner" />
            <span className="pill__proc-label">Processing…</span>
          </div>
        )}
        {state === 'downloading' && (
          <div className="pill__processing">
            <div className="pill__spinner" />
            <span className="pill__proc-label">Downloading model{downloadPct > 0 ? ` ${downloadPct}%` : '…'}</span>
          </div>
        )}
        {state === 'error' && (
          <span className="pill__error-label" title={errorMsg}>Error</span>
        )}
      </div>
    </div>
  )
}
