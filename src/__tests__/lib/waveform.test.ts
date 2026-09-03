import { describe, it, expect } from 'vitest'
import { WAVEFORM_RENDERERS, isCanvasStyle, BARS, type RenderState } from '../../lib/waveform'
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

  it.each(['memo', 'eq', 'spectrum'] as const)('preserves %s history when the card changes width', style => {
    const ctx = new Proxy({} as CanvasRenderingContext2D, { get: () => () => {}, set: () => true })
    const state: RenderState = {}
    const render = WAVEFORM_RENDERERS[style]!
    for (let i = 0; i < 60; i++) render({ ctx, width: 42, height: 20, levels: Array(BARS).fill(.8), dt: 1 / 60, state })
    const before = state[style] as { hist?: number[]; smooth?: number[]; val?: number[] }
    const values = [...(before.hist ?? before.smooth ?? before.val!)]
    render({ ctx, width: 54, height: 20, levels: Array(BARS).fill(.8), dt: 0, state })
    expect(state[style]).toBe(before)
    const after = before.hist ?? before.smooth ?? before.val!
    const mean = (v: number[]) => v.reduce((sum, n) => sum + n, 0) / v.length
    expect(mean(after)).toBeGreaterThan(mean(values) * .9)
  })
})
