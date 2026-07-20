import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'
import type { ThemeName } from './useAppStore'
import type { SettingsTab } from '../lib/routes'

export type BeamSize = 2 | 5 | 8
export type PillTheme = 'dark' | 'steel' | 'light' | 'teal'

export type UiSlice = {
  theme: ThemeName
  pillTheme: PillTheme
  activeRoute: string
  activeSettingsTab: SettingsTab
  modelChosen: boolean
  beamSize: BeamSize
  setTheme: (theme: ThemeName) => void
  setPillTheme: (theme: PillTheme) => void
  setActiveRoute: (route: string) => void
  setActiveSettingsTab: (tab: SettingsTab) => void
  setModelChosen: (chosen: boolean) => void
  setBeamSize: (size: BeamSize) => void
}

export const createUiSlice: StateCreator<AppState, [], [], UiSlice> = (set) => ({
  theme: 'abyss',
  pillTheme: 'dark',
  activeRoute: '/',
  activeSettingsTab: 'appearance',
  modelChosen: false,
  beamSize: 5,

  setTheme: (theme) => set({ theme }),
  setPillTheme: (pillTheme) => set({ pillTheme }),
  setActiveRoute: (route) => set({ activeRoute: route }),
  setActiveSettingsTab: (tab) => set({ activeSettingsTab: tab }),
  setModelChosen: (chosen) => set({ modelChosen: chosen }),
  setBeamSize: (size) => set({ beamSize: size }),
})
