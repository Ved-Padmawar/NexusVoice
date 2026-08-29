import { useEffect, useRef } from 'react'
import { WAVEFORM_RENDERERS, ACCENT_RGB, type RenderState } from '../lib/waveform'
import type { WaveformStyle } from '../store/uiSlice'

type Props = {
  style: WaveformStyle
  width: number
  height: number
  /** Latest capture frame (8 levels, 0–1). Read through a ref each frame. */
  levelsRef: React.RefObject<number[]>
  accent?: string
  /** Faint motion during silence. Settings previews only — see RenderArgs. */
  idleMotion?: boolean
}

/**
 * Drives one canvas waveform style on its own rAF loop.
 *
 * Levels come in through a ref rather than a prop so a 30 Hz capture stream
 * never re-renders React — the loop just reads the newest frame it has.
 */
export function WaveformCanvas({ style, width, height, levelsRef, accent = ACCENT_RGB, idleMotion = false }: Props) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const stateRef = useRef<RenderState>({})

  // A style switch must not inherit the previous style's scroll/peak state.
  useEffect(() => { stateRef.current = {} }, [style, width, height])

  useEffect(() => {
    const canvas = canvasRef.current
    const render = WAVEFORM_RENDERERS[style]
    if (!canvas || !render) return

    const dpr = window.devicePixelRatio || 1
    canvas.width = Math.round(width * dpr)
    canvas.height = Math.round(height * dpr)
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    ctx.scale(dpr, dpr)

    let raf = 0
    let last = performance.now()
    const loop = (now: number) => {
      // Clamp so a backgrounded window doesn't resume with a huge step.
      const dt = Math.min((now - last) / 1000, 0.1)
      last = now
      render({ ctx, width, height, levels: levelsRef.current ?? [], dt, state: stateRef.current, accent, idleMotion })
      raf = requestAnimationFrame(loop)
    }
    raf = requestAnimationFrame(loop)
    return () => cancelAnimationFrame(raf)
  }, [style, width, height, accent, idleMotion, levelsRef])

  return (
    <canvas
      ref={canvasRef}
      style={{ width, height, display: 'block', flex: `0 0 ${width}px` }}
    />
  )
}
