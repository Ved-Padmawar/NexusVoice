import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'
import type { ThemeName } from './useAppStore'
import type { SettingsTab } from '../lib/routes'

export type BeamSize = 2 | 5 | 8

export type UiSlice = {
  theme: ThemeName
  activeRoute: string
  activeSettingsTab: SettingsTab
  modelChosen: boolean
  beamSize: BeamSize
  setTheme: (theme: ThemeName) => void
  setActiveRoute: (route: string) => void
  setActiveSettingsTab: (tab: SettingsTab) => void
  setModelChosen: (chosen: boolean) => void
  setBeamSize: (size: BeamSize) => void
}

export const createUiSlice: StateCreator<AppState, [], [], UiSlice> = (set) => ({
  theme: 'abyss',
  activeRoute: '/',
  activeSettingsTab: 'general',
  modelChosen: false,
  beamSize: 5,

  setTheme: (theme) => set({ theme }),
  setActiveRoute: (route) => set({ activeRoute: route }),
  setActiveSettingsTab: (tab) => set({ activeSettingsTab: tab }),
  setModelChosen: (chosen) => set({ modelChosen: chosen }),
  setBeamSize: (size) => set({ beamSize: size }),
})
