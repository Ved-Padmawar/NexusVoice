import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import { setStoreRef } from './invokeWithRefresh'
import { createAuthSlice, type AuthSlice } from './authSlice'
import { createTranscriptSlice, type TranscriptSlice } from './transcriptSlice'
import { createDictionarySlice, type DictionarySlice } from './dictionarySlice'
import { createModelSlice, type ModelSlice } from './modelSlice'
import { createUiSlice, type UiSlice } from './uiSlice'

export type ThemeName =
  | 'abyss'
  | 'midnight'
  | 'steel'
  | 'pine'
  | 'canvas'
  | 'dawn'
  | 'breeze'
  | 'blossom'

export type AppState = AuthSlice & TranscriptSlice & DictionarySlice & ModelSlice & UiSlice

export type { User, Transcript, DictionaryEntry, UsageStats } from '../types'
export type { PillTheme } from './uiSlice'

export const STORE_PERSIST_KEY = 'nexus-voice-storage'

export const useAppStore = create<AppState>()(
  persist(
    (...args) => ({
      ...createAuthSlice(...args),
      ...createTranscriptSlice(...args),
      ...createDictionarySlice(...args),
      ...createModelSlice(...args),
      ...createUiSlice(...args),
    }),
    {
      name: STORE_PERSIST_KEY,
      // Persist UI-only prefs. `beamSize` is intentionally NOT persisted here —
      // the Rust `beam_size` file is its single source of truth (loaded by AboutTab
      // on mount and read directly by the engine); persisting it too risks drift.
      partialize: (state) => ({ theme: state.theme, pillTheme: state.pillTheme, activeRoute: state.activeRoute, activeSettingsTab: state.activeSettingsTab, modelChosen: state.modelChosen }),
    }
  )
)

// Wire the store reference into invokeWithRefresh so it can access/mutate auth state
setStoreRef(useAppStore)
