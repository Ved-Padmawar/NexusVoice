import { DictionaryEntrySchema, type DictionaryEntry } from '../types'
import { COMMANDS } from '../lib/commands'
import { invokeWithRefresh } from './invokeWithRefresh'
import { toast } from 'sonner'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'

export type DictionarySlice = {
  dictionary: DictionaryEntry[]
  updateDictionary: (term: string, replacement: string) => Promise<void>
  deleteDictionaryEntry: (id: number) => Promise<void>
}

export const createDictionarySlice: StateCreator<AppState, [], [], DictionarySlice> = (set) => ({
  dictionary: [],

  updateDictionary: async (term, replacement) => {
    try {
      const newEntry = DictionaryEntrySchema.parse(
        await invokeWithRefresh<unknown>(COMMANDS.UPDATE_DICTIONARY, { term, replacement })
      )
      set((state) => {
        const index = state.dictionary.findIndex((d) => d.term === term)
        if (index > -1) {
          const newDictionary = [...state.dictionary]
          newDictionary[index] = newEntry
          return { dictionary: newDictionary }
        }
        return { dictionary: [newEntry, ...state.dictionary] }
      })
    } catch (e) {
      toast.error(e instanceof Error ? e.message : 'Failed to update dictionary')
    }
  },

  deleteDictionaryEntry: async (id) => {
    try {
      await invokeWithRefresh<void>(COMMANDS.DELETE_DICTIONARY_ENTRY, { id })
      set((state) => ({ dictionary: state.dictionary.filter((d) => d.id !== id) }))
    } catch (e) {
      toast.error(e instanceof Error ? e.message : 'Failed to delete entry')
    }
  },
})
