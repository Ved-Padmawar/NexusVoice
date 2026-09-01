import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'
import type { ThemeName } from './useAppStore'
import type { SettingsTab } from '../lib/routes'

export type PillTheme = 'steel' | 'midnight' | 'canvas' | 'dawn'

/** How the pill draws the capture levels. All four read the same frame. */
export type WaveformStyle = 'bars' | 'memo' | 'eq' | 'spectrum'

export type UiSlice = {
  theme: ThemeName
  pillTheme: PillTheme
  waveformStyle: WaveformStyle
  /** Expand the pill into a transcript card. Streaming models only. */
  liveTranscript: boolean
  activeRoute: string
  activeSettingsTab: SettingsTab
  modelChosen: boolean
  setTheme: (theme: ThemeName) => void
  setPillTheme: (theme: PillTheme) => void
  setWaveformStyle: (style: WaveformStyle) => void
  setLiveTranscript: (on: boolean) => void
  setActiveRoute: (route: string) => void
  setActiveSettingsTab: (tab: SettingsTab) => void
  setModelChosen: (chosen: boolean) => void
}

export const createUiSlice: StateCreator<AppState, [], [], UiSlice> = (set) => ({
  theme: 'abyss',
  pillTheme: 'steel',
  waveformStyle: 'bars',
  liveTranscript: false,
  activeRoute: '/',
  activeSettingsTab: 'appearance',
  modelChosen: false,

  setTheme: (theme) => set({ theme }),
  setPillTheme: (pillTheme) => set({ pillTheme }),
  setWaveformStyle: (waveformStyle) => set({ waveformStyle }),
  setLiveTranscript: (liveTranscript) => set({ liveTranscript }),
  setActiveRoute: (route) => set({ activeRoute: route }),
  setActiveSettingsTab: (tab) => set({ activeSettingsTab: tab }),
  setModelChosen: (chosen) => set({ modelChosen: chosen }),
})
