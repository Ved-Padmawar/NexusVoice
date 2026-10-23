import { useLayoutEffect, useRef } from 'react'
import { WAVEFORM_RENDERERS, ACCENT_RGB, type RenderState } from '../lib/waveform'
import { subscribeEveryFrame, subscribeWhileVisible } from '../lib/frameClock'
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
  /** Settings previews: draw on the shared ~30 fps clock, and not at all
   *  while off-screen. The pill leaves this off and draws every display frame. */
  preview?: boolean
}

/**
 * Drives one canvas waveform style.
 *
 * Levels come in through a ref rather than a prop so a 30 Hz capture stream
 * never re-renders React — the loop just reads the newest frame it has.
 */
export function WaveformCanvas({ style, width, height, levelsRef, accent = ACCENT_RGB, idleMotion = false, preview = false }: Props) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const stateRef = useRef<RenderState>({})

  // A style switch must not inherit the previous style's scroll/peak state.
  useLayoutEffect(() => { stateRef.current = {} }, [style])

  useLayoutEffect(() => {
    const canvas = canvasRef.current
    const render = WAVEFORM_RENDERERS[style]
    if (!canvas || !render) return

    const dpr = window.devicePixelRatio || 1
    canvas.width = Math.round(width * dpr)
    canvas.height = Math.round(height * dpr)
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    ctx.scale(dpr, dpr)

    const draw = (dt: number) =>
      render({ ctx, width, height, levels: levelsRef.current ?? [], dt, state: stateRef.current, accent, idleMotion })

    // Resizing clears the backing store. Repaint before the browser can show
    // that blank frame, retaining the same waveform history at the new size.
    draw(0)
    return preview ? subscribeWhileVisible(canvas, draw) : subscribeEveryFrame(draw)
  }, [style, width, height, accent, idleMotion, preview, levelsRef])

  return (
    <canvas
      ref={canvasRef}
      style={{ width, height, display: 'block', flex: `0 0 ${width}px` }}
    />
  )
}
