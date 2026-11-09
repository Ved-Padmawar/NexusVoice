import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { queryClient } from '../../app/queryClient'
import { queryKeys } from '../../shared/queryKeys'

// Kept current by mutations and `transcriptSaved`, never by a timer. A refresh
// skipped while hidden runs when the window is shown.
const dictionaryOptions = {
  queryKey: queryKeys.dictionary,
  queryFn: () => commands.getDictionary(),
  staleTime: Infinity,
  refetchOnWindowFocus: true,
}

/** A dictation was saved: its corrections have already bumped the hit counts. */
export const refreshDictionaryUses = () =>
  queryClient.invalidateQueries({ queryKey: queryKeys.dictionary, refetchType: document.hidden ? 'none' : 'active' })

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
    // Returned, so the mutation settles after the list refetches.
    onSuccess: () => client.invalidateQueries({ queryKey: queryKeys.dictionary }),
    onError: (e) => toast.error(extractErrorMessage(e, 'Failed to update dictionary')),
  })
}

export function useDeleteDictionaryEntry() {
  const client = useQueryClient()
  return useMutation({
    mutationFn: (id: number) => commands.deleteDictionaryEntry(id),
    // Returned, so the mutation settles after the list refetches.
    onSuccess: () => client.invalidateQueries({ queryKey: queryKeys.dictionary }),
    onError: (e) => toast.error(extractErrorMessage(e, 'Failed to delete entry')),
  })
}

export const prefetchDictionary = () => queryClient.prefetchQuery(dictionaryOptions)
