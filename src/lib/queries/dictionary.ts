import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { commands } from '../../bindings'
import { extractErrorMessage } from '../errors'
import { queryClient } from './client'
import { queryKeys } from './keys'

const dictionaryOptions = {
  queryKey: queryKeys.dictionary,
  queryFn: () => commands.getDictionary(),
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
    mutationFn: ({ term, replacement, previousTerm }: UpdateDictionaryArgs) =>
      commands.updateDictionary(term, replacement, previousTerm ?? null),
    onSuccess: () => void client.invalidateQueries({ queryKey: queryKeys.dictionary }),
    onError: (e) => toast.error(extractErrorMessage(e, 'Failed to update dictionary')),
  })
}

export function useDeleteDictionaryEntry() {
  const client = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => commands.deleteDictionaryEntry(id),
    onSuccess: () => void client.invalidateQueries({ queryKey: queryKeys.dictionary }),
    onError: (e) => toast.error(extractErrorMessage(e, 'Failed to delete entry')),
  })
}

export const prefetchDictionary = () => queryClient.prefetchQuery(dictionaryOptions)
