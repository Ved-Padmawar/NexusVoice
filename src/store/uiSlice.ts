import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'
import type { ThemeName } from './useAppStore'

export type UiSlice = {
  theme: ThemeName
  activeRoute: string
  activeSettingsTab: 'general' | 'about'
  error: string | null
  setTheme: (theme: ThemeName) => void
  setActiveRoute: (route: string) => void
  setActiveSettingsTab: (tab: 'general' | 'about') => void
  setError: (message: string | null) => void
}

export const createUiSlice: StateCreator<AppState, [], [], UiSlice> = (set) => ({
  theme: 'abyss',
  activeRoute: '/',
  activeSettingsTab: 'general',
  error: null,

  setTheme: (theme) => set({ theme }),
  setActiveRoute: (route) => set({ activeRoute: route }),
  setActiveSettingsTab: (tab) => set({ activeSettingsTab: tab }),
  setError: (message) => set({ error: message }),
})
