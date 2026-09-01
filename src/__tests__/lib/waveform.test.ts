import { describe, it, expect } from 'vitest'
import { WAVEFORM_RENDERERS, isCanvasStyle, BARS } from '../../lib/waveform'
import type { WaveformStyle } from '../../store/uiSlice'

const STYLES: WaveformStyle[] = ['bars', 'memo', 'eq', 'spectrum']

describe('waveform renderers', () => {
  it('has a canvas renderer for every style except bars', () => {
    // `bars` is DOM-drawn, so a renderer for it would be dead code; every
    // other style must have one or the pill renders an empty canvas.
    for (const s of STYLES) {
      expect(Boolean(WAVEFORM_RENDERERS[s])).toBe(isCanvasStyle(s))
    }
  })

  it('renders without touching state it did not initialise', () => {
    // Each renderer lazily seeds its own scratch slot. Reading an unseeded
    // field yields NaN, and a NaN coordinate fails silently on canvas — so
    // a first frame against empty state must still produce draw calls.
    const calls: string[] = []
    const ctx = new Proxy({} as CanvasRenderingContext2D, {
      get: (_t, prop: string) => {
        if (prop === 'canvas') return { width: 42, height: 20 }
        return (...args: unknown[]) => {
          calls.push(prop)
          expect(args.every((a) => typeof a !== 'number' || Number.isFinite(a))).toBe(true)
        }
      },
      set: () => true,
    })

    const levels = new Array(BARS).fill(0.5)
    for (const s of STYLES.filter(isCanvasStyle)) {
      calls.length = 0
      WAVEFORM_RENDERERS[s]!({ ctx, width: 42, height: 20, levels, dt: 0.016, state: {} })
      expect(calls.length, `${s} drew nothing on its first frame`).toBeGreaterThan(0)
    }
  })
})
