import { invoke } from '@tauri-apps/api/core'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { COMMANDS } from '../commands'
import { extractErrorMessage } from '../errors'
import { queryClient } from './client'
import { queryKeys } from './keys'
import type { DictionaryEntry } from '../../types'

const dictionaryOptions = {
  queryKey: queryKeys.dictionary,
  queryFn: () => invoke<DictionaryEntry[]>(COMMANDS.GET_DICTIONARY),
}

export type UpdateDictionaryArgs = {
  term: string
  replacement: string
  previousTerm?: string
}

export function useDictionary() {
  return useQuery(dictionaryOptions)
}

export function useUpdateDictionary() {
  const client = useQueryClient()
  return useMutation({
    mutationFn: (args: UpdateDictionaryArgs) =>
      invoke<DictionaryEntry>(COMMANDS.UPDATE_DICTIONARY, args),
    onSuccess: () => void client.invalidateQueries({ queryKey: queryKeys.dictionary }),
    onError: (e) => toast.error(extractErrorMessage(e, 'Failed to update dictionary')),
  })
}

export function useDeleteDictionaryEntry() {
  const client = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => invoke<void>(COMMANDS.DELETE_DICTIONARY_ENTRY, { id }),
    onSuccess: () => void client.invalidateQueries({ queryKey: queryKeys.dictionary }),
    onError: (e) => toast.error(extractErrorMessage(e, 'Failed to delete entry')),
  })
}

export const prefetchDictionary = () => queryClient.prefetchQuery(dictionaryOptions)
