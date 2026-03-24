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
  | 'nebula'
  | 'pine'
  | 'canvas'
  | 'dawn'
  | 'breeze'
  | 'blossom'

export type AppState = AuthSlice & TranscriptSlice & DictionarySlice & ModelSlice & UiSlice

export type { User, Transcript, DictionaryEntry, UsageStats } from '../types'

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
      name: 'nexus-voice-storage',
      partialize: (state) => ({ theme: state.theme, activeRoute: state.activeRoute, activeSettingsTab: state.activeSettingsTab, modelChosen: state.modelChosen, beamSize: state.beamSize }),
    }
  )
)

// Wire the store reference into invokeWithRefresh so it can access/mutate auth state
setStoreRef(useAppStore)
