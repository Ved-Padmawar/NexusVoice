import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type { PillSettings } from '../../shared/bindings'
import { createAppSlice, type AppSlice } from './appSlice'
import { createCudaSlice, type CudaSlice } from '../../features/models/cudaSlice'
import { createUiSlice, DEFAULT_THEME, THEME_NAMES, type UiSlice } from './uiSlice'
import { createUpdateSlice, type UpdateSlice } from './updateSlice'
import { stashLegacyPill } from './legacyPill'

export const STORE_PERSIST_KEY = 'nexus-voice-storage'

export { THEME_NAMES, type ThemeName } from './uiSlice'

/** State the frontend owns. Backend data lives in the query cache. */
export type AppState = AppSlice & UiSlice & UpdateSlice & CudaSlice

export const useAppStore = create<AppState>()(
  persist(
    (...args) => ({
      ...createAppSlice(...args),
      ...createUiSlice(...args),
      ...createUpdateSlice(...args),
      ...createCudaSlice(...args),
    }),
    {
      name: STORE_PERSIST_KEY,
      // Main-window prefs only. The backend owns the model selection and the
      // pill's preferences, which the pill window reads too.
      partialize: (state) => ({ theme: state.theme, activeRoute: state.activeRoute, activeSettingsTab: state.activeSettingsTab, modelChosen: state.modelChosen, cudaOfferSeen: state.cudaOfferSeen }),
      version: 3,
      migrate: (persisted, from) => {
        const { pillTheme, waveformStyle, liveTranscript, ...rest } = (persisted ?? {}) as Record<string, unknown>
        if (from < 2) {
          // `steps` was replaced by `spectrum`.
          const style = waveformStyle === 'steps' ? 'spectrum' : waveformStyle
          const pill = { theme: pillTheme, waveformStyle: style, liveTranscript }
          const set = Object.entries(pill).filter(([, v]) => v !== undefined)
          if (set.length > 0) stashLegacyPill(Object.fromEntries(set) as Partial<PillSettings>)
        }
        if (from < 3) {
          // Abyss and Pine were retired; a theme with no stylesheet renders unthemed.
          if (rest.theme !== undefined && !(THEME_NAMES as readonly unknown[]).includes(rest.theme)) {
            rest.theme = DEFAULT_THEME
          }
          // The pill's themes moved into Appearance; hotkeys into General.
          if (rest.activeSettingsTab === 'pill') rest.activeSettingsTab = 'appearance'
          if (rest.activeSettingsTab === 'shortcuts') rest.activeSettingsTab = 'general'
        }
        return rest as unknown as AppState
      },
    }
  )
)
