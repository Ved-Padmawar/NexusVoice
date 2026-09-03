/**
 * Canvas renderers for the pill's capture waveform.
 *
 * Every style consumes the same frame the Rust capture thread emits on
 * `pill:waveform` — 8 spectrum levels, 0–1 — so switching styles costs
 * nothing at the backend. `bars` is the original DOM-based style and is
 * drawn by the pill itself; the three here are canvas-drawn.
 *
 * Each renderer owns a scratch object for its scroll/peak state, keyed off
 * the caller-supplied `RenderState`. Callers reuse one state per canvas.
 */

import type { WaveformStyle } from '../store/uiSlice'

export const BARS = 8
export const ACCENT_RGB = '120,162,244'

/** Resting thickness, matching `bars`' MIN_H so silence looks the same in every style. */
export const REST_H = 3

/** Headroom so a loud syllable doesn't pin a style to its ceiling. */
const PEAK_SCALE = 0.8

/** Mic icon height; bottom-anchored styles sit on its foot. */
const ICON_H = 11

/** Per-canvas scratch. Renderers lazily populate their own slot. */
export type RenderState = Record<string, unknown>

export type RenderArgs = {
  ctx: CanvasRenderingContext2D
  /** CSS pixels, not backing-store pixels — the context is pre-scaled. */
  width: number
  height: number
  levels: number[]
  /** Seconds since the previous frame. */
  dt: number
  state: RenderState
  /** Accent as "r,g,b" so themes can recolour the waveform. */
  accent?: string
  /** Faint motion during silence. Previews only — the pill settles flat. */
  idleMotion?: boolean
}

type MemoState = {
  hist: number[]
  scroll: number
  phase: number
  peakSince: number
  n: number
  slot: number
  barW: number
}

type EqState = {
  smooth: number[]
  peak: number[]
  peakVel: number[]
  phase: number
}

type SpectrumState = {
  bins: number
  val: number[]
  seed: number[]
  t: number
}

/** Symmetric bars scrolling right to left — the voice-message look. */
function memo({ ctx, width, height, levels, dt, state, accent = ACCENT_RGB, idleMotion = false }: RenderArgs) {
  ctx.clearRect(0, 0, width, height)

  let s = state.memo as MemoState | undefined
  if (!s) {
    const n = 28
    s = { hist: new Array(n).fill(0), scroll: 0, phase: 0, peakSince: 0, n, slot: width / n, barW: 1.8 }
    state.memo = s
  }
  if (s.slot !== width / s.n) {
    const slot = width / s.n
    s.scroll *= slot / s.slot
    s.slot = slot
  }
  s.phase += dt

  // Hold the peak between emitted bars so a short syllable is never missed.
  s.peakSince = Math.max(s.peakSince, Math.max(...levels))

  s.scroll += 22 * dt
  while (s.scroll >= s.slot) {
    s.scroll -= s.slot
    const idle = idleMotion ? 0.04 + (Math.sin(s.phase * 2.1) * 0.5 + 0.5) * 0.05 : 0
    s.hist.push(Math.max(s.peakSince, idle))
    s.hist.shift()
    s.peakSince = 0
  }

  const midY = height / 2
  ctx.lineCap = 'round'
  ctx.lineWidth = s.barW

  for (let i = 0; i < s.n; i++) {
    const v = s.hist[i]
    const half = Math.max(REST_H / 2, (v * PEAK_SCALE * (height - 2)) / 2)
    const x = i * s.slot + s.slot / 2 - s.scroll
    if (x < -s.barW || x > width + s.barW) continue

    const recency = i / (s.n - 1)
    const alpha = Math.min(1, 0.32 + recency * 0.55 + v * 0.18)
    ctx.strokeStyle = `rgba(${accent},${alpha.toFixed(3)})`
    ctx.beginPath()
    ctx.moveTo(x, midY - half)
    ctx.lineTo(x, midY + half)
    ctx.stroke()
  }

  const leadX = (s.n - 1) * s.slot + s.slot / 2 - s.scroll
  const leadHalf = Math.max(REST_H / 2, (s.hist[s.n - 1] * PEAK_SCALE * (height - 2)) / 2)
  ctx.beginPath()
  ctx.arc(leadX, midY - leadHalf, 1.3, 0, Math.PI * 2)
  ctx.fillStyle = `rgba(${accent},1)`
  ctx.fill()
}

/** Retro stereo equalizer — LED segments with peak-hold dots. */
function eq({ ctx, width, height, levels, dt, state, accent = ACCENT_RGB, idleMotion = false }: RenderArgs) {
  ctx.clearRect(0, 0, width, height)

  let s = state.eq as EqState | undefined
  if (!s) {
    s = {
      smooth: new Array(BARS).fill(0),
      peak: new Array(BARS).fill(0),
      peakVel: new Array(BARS).fill(0),
      phase: 0,
    }
    state.eq = s
  }
  s.phase += dt

  const SEG = 10
  const slot = width / BARS
  const barW = Math.max(1.2, slot - 1.4)
  // Scale the ladder, not just how many rungs light.
  const segH = ((height - 2) * PEAK_SCALE) / SEG
  const midY = height / 2

  for (let i = 0; i < BARS; i++) {
    const rising = levels[i] > s.smooth[i]
    s.smooth[i] += (levels[i] - s.smooth[i]) * (rising ? 0.55 : 0.10 + i * 0.006)

    if (s.smooth[i] > s.peak[i]) {
      s.peak[i] = s.smooth[i]
      s.peakVel[i] = 0
    } else {
      s.peakVel[i] += 0.9 * dt
      s.peak[i] -= s.peakVel[i] * dt
      if (s.peak[i] < s.smooth[i]) {
        s.peak[i] = s.smooth[i]
        s.peakVel[i] = 0
      }
    }
  }

  for (let i = 0; i < BARS; i++) {
    const cx = i * slot + slot / 2
    const litCount = s.smooth[i] * SEG
    const idle = idleMotion ? 0.10 + (Math.sin(s.phase * 2.0 + i * 0.6) * 0.5 + 0.5) * 0.10 : 0
    // 0..1, the scale `segMid` is on — a 0..SEG count lights every segment.
    const baseLit = Math.max(s.smooth[i], idle)

    // The segment grid goes dark at zero, which reads as broken.
    ctx.fillStyle = `rgba(${accent},0.85)`
    ctx.fillRect(cx - barW / 2, midY - REST_H / 2, barW, REST_H)

    for (let seg = 0; seg < SEG; seg++) {
      const segMid = (seg + 0.5) / SEG
      const lit = baseLit >= segMid + 0.5 / SEG
      const halfLit = !lit && baseLit > segMid - 0.5 / SEG
      if (!lit && !halfLit) continue

      const half = ((seg + 1) * segH) / 2
      const alpha = lit ? 1 : (baseLit - segMid) * SEG * 0.9
      ctx.fillStyle = `rgba(${accent},${(0.85 * alpha).toFixed(3)})`
      ctx.fillRect(cx - barW / 2, midY - half, barW, segH * 0.78)
      ctx.fillRect(cx - barW / 2, midY + half - segH * 0.78, barW, segH * 0.78)
    }

    if (s.peak[i] * SEG > 0.05) {
      const peakHalf = (s.peak[i] * PEAK_SCALE * (height - 2)) / 2
      ctx.fillStyle = `rgba(${accent},1)`
      ctx.beginPath()
      ctx.arc(cx, midY - peakHalf, 1.1, 0, Math.PI * 2)
      ctx.fill()
      ctx.beginPath()
      ctx.arc(cx, midY + peakHalf, 1.1, 0, Math.PI * 2)
      ctx.fill()
    }

    if (litCount > 0.5) {
      ctx.beginPath()
      ctx.arc(cx, midY - Math.min(litCount, SEG) * segH, 2.2, 0, Math.PI * 2)
      ctx.fillStyle = `rgba(${accent},0.18)`
      ctx.fill()
    }
  }
}

/** Analyser columns, low frequencies left. The 8 bands are interpolated across
 *  a finer grid and jittered per column, or they move in lockstep as blocks. */
function spectrum({ ctx, width, height, levels, dt, state, accent = ACCENT_RGB, idleMotion = false }: RenderArgs) {
  ctx.clearRect(0, 0, width, height)

  const BINS = Math.max(12, Math.min(30, Math.round(width / 3)))
  let s = state.spectrum as SpectrumState | undefined
  if (!s) {
    s = {
      bins: BINS,
      val: new Array(BINS).fill(0),
      seed: Array.from({ length: BINS }, () => Math.random() * Math.PI * 2),
      t: 0,
    }
    state.spectrum = s
  }
  if (s.bins !== BINS) {
    const resample = (values: number[]) => Array.from({ length: BINS }, (_, i) => {
      const x = i * (values.length - 1) / (BINS - 1)
      const left = Math.floor(x)
      return values[left] + (values[Math.min(left + 1, values.length - 1)] - values[left]) * (x - left)
    })
    s.val = resample(s.val)
    s.seed = resample(s.seed)
    s.bins = BINS
  }
  s.t += dt

  const bandAt = (t: number) => {
    const x = t * (BARS - 1)
    const i = Math.min(BARS - 2, Math.floor(x))
    const f = x - i
    const v = (levels[i] ?? 0) * (1 - f) + (levels[i + 1] ?? 0) * f
    return v * (1 + t * 1.15)
  }

  const slot = width / BINS
  const colW = Math.max(1.4, Math.min(2.6, slot * 0.55))
  const r = colW / 2
  const floorY = height / 2 + ICON_H / 2
  const maxH = (floorY - 1) * PEAK_SCALE

  ctx.fillStyle = `rgba(${accent},0.92)`

  for (let i = 0; i < BINS; i++) {
    const t = BINS === 1 ? 0 : i / (BINS - 1)
    const jitter = 0.72 + (Math.sin(s.t * 9 + s.seed[i]) * 0.5 + 0.5) * 0.55
    const idle = idleMotion ? 0.05 + (Math.sin(s.t * 1.8 + i * 0.4) * 0.5 + 0.5) * 0.06 : 0
    const target = Math.min(1, Math.max(bandAt(t) * jitter, idle))
    const rising = target > s.val[i]
    s.val[i] += (target - s.val[i]) * (rising ? 0.55 : 0.16)

    const x = i * slot + (slot - colW) / 2
    const h = Math.max(REST_H, s.val[i] * maxH)

    ctx.beginPath()
    ctx.roundRect(x, floorY - h, colW, h, [r, r, 0, 0])
    ctx.fill()
  }
}

/** Canvas-drawn styles. `bars` is DOM-based and has no entry here. */
export const WAVEFORM_RENDERERS: Partial<Record<WaveformStyle, (a: RenderArgs) => void>> = {
  memo,
  eq,
  spectrum,
}

export function isCanvasStyle(style: WaveformStyle): boolean {
  return style !== 'bars'
}
