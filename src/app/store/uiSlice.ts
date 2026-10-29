import type { StateCreator } from 'zustand'
import { commands, type PillSettings, type PillSettingsSnapshot } from '../../shared/bindings'
import { logger } from '../../shared/logger'
import { subscribePillSettings } from '../../pill/pillSettings'
import { takeLegacyPill } from './legacyPill'
import type { AppState } from './useAppStore'
import type { SettingsTab } from '../routes'

export const THEME_NAMES = ['steel', 'midnight', 'indigo', 'orchid', 'canvas', 'dawn', 'breeze', 'blossom'] as const
export type ThemeName = typeof THEME_NAMES[number]
export const DEFAULT_THEME: ThemeName = 'indigo'

const DEFAULT_PILL: PillSettings = { theme: 'steel', waveformStyle: 'bars', liveTranscript: false, startSound: false }

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

export const createUiSlice: StateCreator<AppState, [], [], UiSlice> = (set, get) => {
  let savesPending = 0
  let newest: PillSettingsSnapshot | undefined

  // Only a snapshot newer than any seen may apply, so a delayed event never
  // replays an older edit.
  const isNewest = (snapshot: PillSettingsSnapshot) => {
    if (newest && snapshot.revision <= newest.revision) return false
    newest = snapshot
    return true
  }

  return {
    theme: DEFAULT_THEME,
    pill: DEFAULT_PILL,
    activeRoute: '/',
    activeSettingsTab: 'general',
    modelChosen: false,

    setTheme: (theme) => set({ theme }),
    // Applied at once so the picker responds; snapshots wait out pending saves.
    updatePill: async (change) => {
      const next = { ...get().pill, ...change }
      set({ pill: next })
      savesPending += 1
      try {
        isNewest(await commands.setPillSettings(next))
      } catch (e) {
        logger.error('could not save pill settings', e)
      }
      savesPending -= 1
      if (savesPending === 0) {
        newest ??= await commands.getPillSettings()
        set({ pill: newest.settings })
      }
    },
    followPillSettings: () => {
      const stop = subscribePillSettings((settings, revision) => {
        if (isNewest({ revision, settings }) && savesPending === 0) set({ pill: settings })
      })
      const legacy = takeLegacyPill()
      if (legacy) void get().updatePill(legacy)
      return stop
    },
    setActiveRoute: (route) => set({ activeRoute: route }),
    setActiveSettingsTab: (tab) => set({ activeSettingsTab: tab }),
    setModelChosen: (chosen) => set({ modelChosen: chosen }),
  }
}
