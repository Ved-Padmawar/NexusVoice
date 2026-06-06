import { z } from 'zod'
import { DictionaryEntrySchema, type DictionaryEntry } from '../types'
import { COMMANDS } from '../lib/commands'
import { invokeWithRefresh } from './invokeWithRefresh'
import { toast } from 'sonner'
import type { StateCreator } from 'zustand'
import type { AppState } from './useAppStore'
import type { AsyncStatus } from './asyncStatus'

export type DictionarySlice = {
  dictionary: DictionaryEntry[]
  dictionaryStatus: AsyncStatus
  dictionaryError: string | null
  loadDictionary: () => Promise<void>
  updateDictionary: (term: string, replacement: string) => Promise<void>
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
        await invokeWithRefresh<unknown>(COMMANDS.GET_DICTIONARY)
      )
      set({ dictionary, dictionaryStatus: 'success' })
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Failed to load dictionary'
      set({ dictionaryStatus: 'error', dictionaryError: message })
    }
  },

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
