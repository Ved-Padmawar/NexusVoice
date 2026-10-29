/**
 * Pill theme values, mirrored from `.pill[data-pill-theme=…]` in
 * `src/pill/PillApp.css`. The pill window styles itself from that CSS; the
 * settings previews and the canvas waveforms need the same values in JS,
 * so this is the one place both read from.
 */

import type { PillTheme } from '../shared/types'

export type PillThemeDef = {
  id: PillTheme
  label: string
  blurb: string
  /** Any CSS background: a colour or a gradient. */
  bg: string
  border: string
  /** Mic icon and waveform colour, "#rrggbb". */
  accent: string
  /** `accent` as "r,g,b", derived — canvas fills need the channels separately. */
  accentRgb: string
  brand: string
  /** Backdrop for the settings previews. */
  stage: string
}

/** "#rrggbb" to the "r,g,b" triple a `rgba()` fill needs. */
function accentChannels(hex: string): string {
  const h = hex.replace('#', '')
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16)).join(',')
}

export const PILL_THEMES: PillThemeDef[] = ([
  { id: 'steel',    label: 'Steel',    blurb: 'Graphite, quiet',    bg: '#141820', border: 'rgba(148,168,200,0.15)', accent: '#b8cce0', brand: 'rgba(222,230,240,0.88)', stage: 'color-mix(in srgb, #141820 60%, #111)' },
  { id: 'midnight', label: 'Midnight', blurb: 'Ink and cyan', bg: '#0a0d14', border: 'rgba(26,209,209,0.16)',  accent: '#22d3d3', brand: 'rgba(236,238,244,0.90)', stage: 'color-mix(in srgb, #0a0d14 60%, #111)' },
  { id: 'aurora',   label: 'Aurora',   blurb: 'Sky into ember',   bg: 'linear-gradient(105deg in oklch, #4fa8dc 0%, #6a50d6 42%, #e2553b 100%)', border: 'rgba(255,255,255,0.30)', accent: '#ffffff', brand: 'rgba(255,255,255,0.98)', stage: 'color-mix(in srgb, #3a2f7a 60%, #111)' },
  { id: 'canvas',   label: 'Canvas',   blurb: 'Porcelain blue',   bg: 'linear-gradient(180deg, #f6f8ff 0%, #e3e9fb 100%)', border: '#93a5e9', accent: '#3456d6', brand: '#18204a', stage: '#e8eaf0' },
  { id: 'dawn',     label: 'Dawn',     blurb: 'Apricot and terracotta',     bg: 'linear-gradient(180deg, #ffeedf 0%, #ffd3bb 100%)', border: '#e0936c', accent: '#c2410c', brand: '#3b1a0a', stage: '#efe4da' },
  { id: 'glass',    label: 'Glass',    blurb: 'Smoked frost',    bg: 'linear-gradient(180deg, rgba(255,255,255,0.22), rgba(255,255,255,0.06)), rgba(24,28,38,0.32)', border: 'rgba(255,255,255,0.32)', accent: '#ffffff', brand: 'rgba(255,255,255,0.96)', stage: '#3a4150' },
] satisfies Omit<PillThemeDef, 'accentRgb'>[]).map((t) => ({ ...t, accentRgb: accentChannels(t.accent) }))

export function pillThemeDef(id: PillTheme): PillThemeDef {
  return PILL_THEMES.find((t) => t.id === id) ?? PILL_THEMES[0]
}
