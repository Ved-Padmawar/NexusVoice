import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback } from 'react'
import { toast } from 'sonner'
import { commands, type DictionaryResponse } from '../../shared/bindings'
import { createPendingDeletes, deleteWithUndo } from '../../shared/deleteWithUndo'
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
  /** The entry being edited; absent for a new one. */
  id?: number
}

const pendingDeletes = createPendingDeletes()

export function useDictionary() {
  const ids = pendingDeletes.useIds()
  const select = useCallback(
    (list: DictionaryResponse[]) => (ids.size ? list.filter(entry => !ids.has(entry.id)) : list),
    [ids],
  )
  return useQuery({ ...dictionaryOptions, select })
}

export function useUpdateDictionary() {
  const client = useQueryClient()
  return useMutation({
    mutationFn: ({ term, replacement, id }: UpdateDictionaryArgs) =>
      commands.updateDictionary(term, replacement, id ?? null),
    // Returned, so the mutation settles after the list refetches.
    onSuccess: () => client.invalidateQueries({ queryKey: queryKeys.dictionary }),
    onError: (e) => toast.error(extractErrorMessage(e, 'Failed to update dictionary')),
  })
}

/** Deletes an entry behind an Undo toast. */
export function useDeleteDictionaryEntry() {
  const client = useQueryClient()
  const { mutateAsync } = useMutation({
    mutationFn: (id: number) => commands.deleteDictionaryEntry(id),
    // Dropped first: an unmounted list is only marked stale. Returned, so the
    // entry stays hidden until the refetch lands.
    onSuccess: (_, id) => {
      client.setQueryData<DictionaryResponse[]>(queryKeys.dictionary, list => list?.filter(e => e.id !== id))
      return client.invalidateQueries({ queryKey: queryKeys.dictionary })
    },
    onError: (e) => toast.error(extractErrorMessage(e, 'Failed to delete entry')),
  })
  return (entry: DictionaryResponse) => deleteWithUndo({
    message: `“${entry.term}” deleted`,
    id: entry.id,
    pending: pendingDeletes,
    commit: () => mutateAsync(entry.id),
  })
}

export const prefetchDictionary = () => queryClient.prefetchQuery(dictionaryOptions)
