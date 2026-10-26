import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type { PillSettings } from '../bindings'
import { createAppSlice, type AppSlice } from './appSlice'
import { createCudaSlice, type CudaSlice } from './cudaSlice'
import { createModelSlice, type ModelSlice } from './modelSlice'
import { createUiSlice, type UiSlice } from './uiSlice'
import { createUpdateSlice, type UpdateSlice } from './updateSlice'
import { stashLegacyPill } from './legacyPill'

export const STORE_PERSIST_KEY = 'nexus-voice-storage'

export type ThemeName =
  | 'abyss'
  | 'midnight'
  | 'steel'
  | 'pine'
  | 'canvas'
  | 'dawn'
  | 'breeze'
  | 'blossom'

export type AppState = AppSlice & ModelSlice & UiSlice & UpdateSlice & CudaSlice

export const useAppStore = create<AppState>()(
  persist(
    (...args) => ({
      ...createAppSlice(...args),
      ...createModelSlice(...args),
      ...createUiSlice(...args),
      ...createUpdateSlice(...args),
      ...createCudaSlice(...args),
    }),
    {
      name: STORE_PERSIST_KEY,
      // Main-window prefs only. The backend owns the model selection and the
      // pill's preferences, which the pill window reads too.
      partialize: (state) => ({ theme: state.theme, activeRoute: state.activeRoute, activeSettingsTab: state.activeSettingsTab, modelChosen: state.modelChosen, cudaOfferSeen: state.cudaOfferSeen }),
      version: 2,
      migrate: (persisted, from) => {
        const { pillTheme, waveformStyle, liveTranscript, ...rest } = (persisted ?? {}) as Record<string, unknown>
        if (from < 2) {
          // `steps` was replaced by `spectrum`.
          const style = waveformStyle === 'steps' ? 'spectrum' : waveformStyle
          const pill = { theme: pillTheme, waveformStyle: style, liveTranscript }
          const set = Object.entries(pill).filter(([, v]) => v !== undefined)
          if (set.length > 0) stashLegacyPill(Object.fromEntries(set) as Partial<PillSettings>)
        }
        return rest as unknown as AppState
      },
    }
  )
)
