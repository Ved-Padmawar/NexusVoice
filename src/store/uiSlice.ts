import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'
import type { ThemeName } from './useAppStore'
import type { SettingsTab } from '../lib/routes'

export type PillTheme = 'steel' | 'midnight' | 'canvas' | 'dawn'

export type UiSlice = {
  theme: ThemeName
  pillTheme: PillTheme
  activeRoute: string
  activeSettingsTab: SettingsTab
  modelChosen: boolean
  setTheme: (theme: ThemeName) => void
  setPillTheme: (theme: PillTheme) => void
  setActiveRoute: (route: string) => void
  setActiveSettingsTab: (tab: SettingsTab) => void
  setModelChosen: (chosen: boolean) => void
}

export const createUiSlice: StateCreator<AppState, [], [], UiSlice> = (set) => ({
  theme: 'abyss',
  pillTheme: 'steel',
  activeRoute: '/',
  activeSettingsTab: 'appearance',
  modelChosen: false,

  setTheme: (theme) => set({ theme }),
  setPillTheme: (pillTheme) => set({ pillTheme }),
  setActiveRoute: (route) => set({ activeRoute: route }),
  setActiveSettingsTab: (tab) => set({ activeSettingsTab: tab }),
  setModelChosen: (chosen) => set({ modelChosen: chosen }),
})
