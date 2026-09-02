import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import { createAppSlice, type AppSlice } from './appSlice'
import { createModelSlice, type ModelSlice } from './modelSlice'
import { createUiSlice, type UiSlice } from './uiSlice'
import { createUpdateSlice, type UpdateSlice } from './updateSlice'
import { STORE_PERSIST_KEY } from './persistKey'

export type ThemeName =
  | 'abyss'
  | 'midnight'
  | 'steel'
  | 'pine'
  | 'canvas'
  | 'dawn'
  | 'breeze'
  | 'blossom'

export type AppState = AppSlice & ModelSlice & UiSlice & UpdateSlice

export type { PillTheme } from './uiSlice'

export const useAppStore = create<AppState>()(
  persist(
    (...args) => ({
      ...createAppSlice(...args),
      ...createModelSlice(...args),
      ...createUiSlice(...args),
      ...createUpdateSlice(...args),
    }),
    {
      name: STORE_PERSIST_KEY,
      // UI-only prefs. Model selection is not persisted here — the Rust
      // `model_override` file is its single source of truth.
      partialize: (state) => ({ theme: state.theme, pillTheme: state.pillTheme, waveformStyle: state.waveformStyle, activeRoute: state.activeRoute, activeSettingsTab: state.activeSettingsTab, modelChosen: state.modelChosen, liveTranscript: state.liveTranscript }),
      version: 1,
      migrate: (persisted, from) => {
        const s = persisted as Partial<AppState>
        // `steps` was replaced by `spectrum`; leaving it set renders nothing.
        if (from < 1 && (s.waveformStyle as string) === 'steps') s.waveformStyle = 'spectrum'
        return s as AppState
      },
    }
  )
)
