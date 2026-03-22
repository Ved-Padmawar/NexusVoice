import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'
import type { ThemeName } from './useAppStore'
import type { SettingsTab } from '../lib/routes'

export type UiSlice = {
  theme: ThemeName
  activeRoute: string
  activeSettingsTab: SettingsTab
  modelChosen: boolean
  setTheme: (theme: ThemeName) => void
  setActiveRoute: (route: string) => void
  setActiveSettingsTab: (tab: SettingsTab) => void
  setModelChosen: (chosen: boolean) => void
}

export const createUiSlice: StateCreator<AppState, [], [], UiSlice> = (set) => ({
  theme: 'abyss',
  activeRoute: '/',
  activeSettingsTab: 'general',
  modelChosen: false,

  setTheme: (theme) => set({ theme }),
  setActiveRoute: (route) => set({ activeRoute: route }),
  setActiveSettingsTab: (tab) => set({ activeSettingsTab: tab }),
  setModelChosen: (chosen) => set({ modelChosen: chosen }),
})
