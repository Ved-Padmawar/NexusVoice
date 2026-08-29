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

type StepsState = {
  hist: number[]
  scroll: number
  phase: number
  peakSince: number
  flick: number[]
  flickRate: number[]
}

/** Symmetric bars scrolling right to left — the voice-message look. */
function memo({ ctx, width, height, levels, dt, state, accent = ACCENT_RGB, idleMotion = false }: RenderArgs) {
  ctx.clearRect(0, 0, width, height)

  let s = state.memo as MemoState | undefined
  if (!s || s.slot !== width / 28) {
    const n = 28
    s = { hist: new Array(n).fill(0), scroll: 0, phase: 0, peakSince: 0, n, slot: width / n, barW: 1.8 }
    state.memo = s
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
    const half = Math.max(REST_H / 2, (v * (height - 2)) / 2)
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
  const leadHalf = Math.max(REST_H / 2, (s.hist[s.n - 1] * (height - 2)) / 2)
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

  const SEG = 6
  const slot = width / BARS
  const barW = Math.max(1.2, slot - 1.4)
  const segH = (height - 2) / SEG
  const midY = height / 2

  for (let i = 0; i < BARS; i++) {
    const rising = levels[i] > s.smooth[i]
    s.smooth[i] += (levels[i] - s.smooth[i]) * (rising ? 0.55 : 0.10 + i * 0.006)

    if (s.smooth[i] > s.peak[i]) {
      s.peak[i] = s.smooth[i]
      s.peakVel[i] = 0
    } else {
      s.peakVel[i] += 0.25 * dt
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
    const baseLit = Math.max(litCount, idle)

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
      const peakHalf = (s.peak[i] * (height - 2)) / 2
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

/** Chunky pixel silhouette scrolling right to left. */
function steps({ ctx, width, height, levels, dt, state, accent = ACCENT_RGB, idleMotion = false }: RenderArgs) {
  ctx.clearRect(0, 0, width, height)

  const PX = 2
  const cols = Math.floor(width / PX)
  const rows = Math.floor(height / PX)
  const half = rows / 2
  const maxHalf = half - 1

  let s = state.steps as StepsState | undefined
  if (!s || s.hist.length !== cols) {
    s = {
      hist: new Array(cols).fill(0),
      scroll: 0,
      phase: 0,
      peakSince: 0,
      flick: Array.from({ length: cols }, (_, i) => i * 0.5),
      flickRate: Array.from({ length: cols }, (_, i) => 2.5 + (i % 4) * 0.7),
    }
    state.steps = s
  }

  s.phase += dt
  for (let i = 0; i < cols; i++) s.flick[i] += dt * s.flickRate[i]
  s.peakSince = Math.max(s.peakSince, Math.max(...levels))

  s.scroll += 26 * dt
  while (s.scroll >= PX) {
    s.scroll -= PX
    const idle = idleMotion ? 0.05 + (Math.sin(s.phase * 1.6) * 0.5 + 0.5) * 0.05 : 0
    s.hist.push(Math.max(s.peakSince, idle))
    s.hist.shift()
    s.peakSince = 0
  }

  for (let i = 0; i < cols; i++) {
    const x = i * PX
    const v = s.hist[i]
    const recency = i / (cols - 1)
    const flicker = 0.88 + Math.sin(s.flick[i]) * 0.12
    const baseAlpha = Math.min(1, (0.32 + recency * 0.5 + v * 0.25) * flicker)

    const idle = idleMotion ? 0.05 + (Math.sin(s.phase * 1.4 + i * 0.3) * 0.5 + 0.5) * 0.05 : 0
    // Rests at 4px, not REST_H: a 2px grid can't express 3.
    const halfRows = Math.round(Math.max(v, idle) * maxHalf)

    for (let r = 0; r <= halfRows; r++) {
      const isRim = r === halfRows && halfRows > 0
      const alpha = isRim ? Math.min(1, baseAlpha + 0.25) : baseAlpha
      ctx.fillStyle = `rgba(${accent},${alpha.toFixed(3)})`
      ctx.fillRect(x, (half - 1 - r) * PX, PX, PX)
      ctx.fillRect(x, (half + r) * PX, PX, PX)
    }
  }
}

/** Canvas-drawn styles. `bars` is DOM-based and has no entry here. */
export const WAVEFORM_RENDERERS: Partial<Record<WaveformStyle, (a: RenderArgs) => void>> = {
  memo,
  eq,
  steps,
}

export function isCanvasStyle(style: WaveformStyle): boolean {
  return style !== 'bars'
}
