import type { ThemeName } from '../store/useAppStore'

export type ThemeMeta = {
  name: ThemeName
  label: string
  blurb: string
  mode: 'dark' | 'light'
}

/** Display metadata; the colours live in `styles/themes.css`. */
export const THEMES: ThemeMeta[] = [
  { name: 'abyss',    label: 'Abyss',    blurb: 'Deep blue, periwinkle', mode: 'dark' },
  { name: 'midnight', label: 'Midnight', blurb: 'Near black, cyan',      mode: 'dark' },
  { name: 'steel',    label: 'Steel',    blurb: 'Slate grey, ice blue',  mode: 'dark' },
  { name: 'pine',     label: 'Pine',     blurb: 'Forest green, mint',    mode: 'dark' },
  { name: 'canvas',   label: 'Canvas',   blurb: 'Clean white, cobalt',   mode: 'light' },
  { name: 'dawn',     label: 'Dawn',     blurb: 'Warm white, orange',    mode: 'light' },
  { name: 'breeze',   label: 'Breeze',   blurb: 'Pale teal, teal',       mode: 'light' },
  { name: 'blossom',  label: 'Blossom',  blurb: 'Soft rose, rose',       mode: 'light' },
]

export function themeMeta(name: ThemeName): ThemeMeta {
  return THEMES.find((t) => t.name === name) ?? THEMES[0]
}
