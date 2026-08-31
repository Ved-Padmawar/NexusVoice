import { z } from 'zod'
import { DictionaryEntrySchema, type DictionaryEntry } from '../types'
import { COMMANDS } from '../lib/commands'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'
import type { AsyncStatus } from './asyncStatus'
import { extractErrorMessage } from '../lib/errors'

export type DictionarySlice = {
  dictionary: DictionaryEntry[]
  dictionaryStatus: AsyncStatus
  dictionaryError: string | null
  loadDictionary: () => Promise<void>
  updateDictionary: (term: string, replacement: string, previousTerm?: string) => Promise<void>
  deleteDictionaryEntry: (id: number) => Promise<void>
}

export const createDictionarySlice: StateCreator<AppState, [], [], DictionarySlice> = (set, get) => ({
  dictionary: [],
  dictionaryStatus: 'idle',
  dictionaryError: null,

  loadDictionary: async () => {
    if (!get().user) return
    set({ dictionaryStatus: 'loading', dictionaryError: null })
    try {
      const dictionary = z.array(DictionaryEntrySchema).parse(
        await invoke<unknown>(COMMANDS.GET_DICTIONARY)
      )
      set({ dictionary, dictionaryStatus: 'success' })
    } catch (e) {
      const message = extractErrorMessage(e, 'Failed to load dictionary')
      set({ dictionaryStatus: 'error', dictionaryError: message })
    }
  },

  updateDictionary: async (term, replacement, previousTerm) => {
    try {
      const newEntry = DictionaryEntrySchema.parse(
        await invoke<unknown>(COMMANDS.UPDATE_DICTIONARY, { term, replacement, previousTerm })
      )
      set((state) => {
        // A rename replaces the row it renamed, so match the old term too.
        const index = state.dictionary.findIndex(
          (d) => d.term === term || (previousTerm !== undefined && d.term === previousTerm),
        )
        if (index > -1) {
          const newDictionary = [...state.dictionary]
          newDictionary[index] = newEntry
          return { dictionary: newDictionary }
        }
        return { dictionary: [newEntry, ...state.dictionary] }
      })
    } catch (e) {
      toast.error(extractErrorMessage(e, 'Failed to update dictionary'))
    }
  },

  deleteDictionaryEntry: async (id) => {
    try {
      await invoke<void>(COMMANDS.DELETE_DICTIONARY_ENTRY, { id })
      set((state) => ({ dictionary: state.dictionary.filter((d) => d.id !== id) }))
    } catch (e) {
      toast.error(extractErrorMessage(e, 'Failed to delete entry'))
    }
  },
})
