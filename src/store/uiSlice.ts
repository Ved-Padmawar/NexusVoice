import type { StateCreator } from 'zustand'
import { commands, type PillSettings } from '../bindings'
import { logger } from '../lib/logger'
import { subscribePillSettings } from '../lib/pillSettings'
import { takeLegacyPill } from './legacyPill'
import type { AppState, ThemeName } from './useAppStore'
import type { SettingsTab } from '../lib/routes'

const DEFAULT_PILL: PillSettings = { theme: 'steel', waveformStyle: 'bars', liveTranscript: false }

export type UiSlice = {
  theme: ThemeName
  /** Mirrors the backend's saved pill preferences, which the pill also reads. */
  pill: PillSettings
  activeRoute: string
  activeSettingsTab: SettingsTab
  modelChosen: boolean
  setTheme: (theme: ThemeName) => void
  updatePill: (change: Partial<PillSettings>) => Promise<void>
  followPillSettings: () => () => void
  setActiveRoute: (route: string) => void
  setActiveSettingsTab: (tab: SettingsTab) => void
  setModelChosen: (chosen: boolean) => void
}

export const createUiSlice: StateCreator<AppState, [], [], UiSlice> = (set, get) => ({
  theme: 'abyss',
  pill: DEFAULT_PILL,
  activeRoute: '/',
  activeSettingsTab: 'appearance',
  modelChosen: false,

  setTheme: (theme) => set({ theme }),
  // Applied at once so the picker responds; the saved snapshot follows.
  updatePill: async (change) => {
    const next = { ...get().pill, ...change }
    set({ pill: next })
    try {
      await commands.setPillSettings(next)
    } catch (e) {
      logger.error('could not save pill settings', e)
      set({ pill: (await commands.getPillSettings()).settings })
    }
  },
  followPillSettings: () => {
    const stop = subscribePillSettings((pill) => set({ pill }))
    const legacy = takeLegacyPill()
    if (legacy) void get().updatePill(legacy)
    return stop
  },
  setActiveRoute: (route) => set({ activeRoute: route }),
  setActiveSettingsTab: (tab) => set({ activeSettingsTab: tab }),
  setModelChosen: (chosen) => set({ modelChosen: chosen }),
})
