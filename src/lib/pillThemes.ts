/**
 * Pill theme values, mirrored from `.pill[data-pill-theme=…]` in
 * `src/pill/PillApp.css`. The pill window styles itself from that CSS; the
 * settings previews and the canvas waveforms need the same values in JS,
 * so this is the one place both read from.
 */

import type { PillTheme } from '../store/uiSlice'

export type PillThemeDef = {
  id: PillTheme
  label: string
  bg: string
  border: string
  /** Mic icon and waveform colour. */
  accent: string
  /** `accent` as "r,g,b", derived — canvas fills need the channels separately. */
  accentRgb: string
  brand: string
}

/** "#rrggbb" to the "r,g,b" triple a `rgba()` fill needs. */
function accentChannels(hex: string): string {
  const h = hex.replace('#', '')
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16)).join(',')
}

export const PILL_THEMES: PillThemeDef[] = ([
  { id: 'steel',    label: 'Steel',    bg: '#141820', border: 'rgba(148,168,200,0.15)', accent: '#b8cce0', brand: 'rgba(200,215,235,0.82)' },
  { id: 'midnight', label: 'Midnight', bg: '#0a0d14', border: 'rgba(26,209,209,0.16)',  accent: '#1ad1d1', brand: 'rgba(236,238,244,0.82)' },
  { id: 'canvas',   label: 'Canvas',   bg: '#ffffff', border: 'rgba(0,0,0,0.10)',       accent: '#3a5bd9', brand: 'rgba(20,20,45,0.82)'    },
  { id: 'dawn',     label: 'Dawn',     bg: '#fff6f4', border: 'rgba(120,60,40,0.14)',   accent: '#e43800', brand: 'rgba(37,22,20,0.82)'    },
] satisfies Omit<PillThemeDef, 'accentRgb'>[]).map((t) => ({ ...t, accentRgb: accentChannels(t.accent) }))

export function pillThemeDef(id: PillTheme): PillThemeDef {
  return PILL_THEMES.find((t) => t.id === id) ?? PILL_THEMES[0]
}
