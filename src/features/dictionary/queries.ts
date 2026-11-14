import { useMutation, useQuery } from '@tanstack/react-query'
import { useCallback } from 'react'
import { toast } from 'sonner'
import { commands, type DictionaryResponse } from '../../shared/bindings'
import { createPendingDeletes, deleteWithUndo } from '../../shared/deleteWithUndo'
import { extractErrorMessage } from '../../shared/errors'
import { queryClient } from '../../app/queryClient'
import { caughtUp, reads } from '../../app/sync'
import { queryKeys } from '../../shared/queryKeys'

const dictionaryOptions = {
  queryKey: queryKeys.dictionary,
  queryFn: () => commands.getDictionary(),
  ...reads('dictionary'),
}

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

/** Settles once the list shows the save, so an edited row never flashes its old text. */
export function useUpdateDictionary() {
  return useMutation({
    mutationFn: async ({ term, replacement, id }: UpdateDictionaryArgs) => {
      await commands.updateDictionary(term, replacement, id ?? null)
      await caughtUp('dictionary')
    },
    onError: (e) => toast.error(extractErrorMessage(e, 'Failed to update dictionary')),
  })
}

/** Deletes an entry behind an Undo toast. */
export function useDeleteDictionaryEntry() {
  const { mutateAsync } = useMutation({
    mutationFn: (id: number) => commands.deleteDictionaryEntry(id),
    onError: (e) => toast.error(extractErrorMessage(e, 'Failed to delete entry')),
  })
  return (entry: DictionaryResponse) => deleteWithUndo({
    message: `“${entry.term}” deleted`,
    id: entry.id,
    pending: pendingDeletes,
    commit: () => mutateAsync(entry.id),
  })
}

export const prefetchDictionary = () => queryClient.query(dictionaryOptions).catch(() => {})
