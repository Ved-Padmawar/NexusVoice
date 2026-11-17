import { useMutation } from '@tanstack/react-query'
import { useMemo } from 'react'
import { toast } from 'sonner'
import { commands, type DictionaryResponse } from '../../shared/bindings'
import { createPendingDeletes, deleteWithUndo } from '../../shared/deleteWithUndo'
import { extractErrorMessage } from '../../shared/errors'
import { mirrored, useSlice } from '../../app/mirror'

export type UpdateDictionaryArgs = {
  term: string
  replacement: string
  /** The entry being edited; absent for a new one. */
  id?: number
}

const pendingDeletes = createPendingDeletes()

/** Every entry, sorted by term, without those waiting out their undo window. */
export function useDictionary() {
  const ids = pendingDeletes.useIds()
  const entries = useSlice('dictionary', dictionary => dictionary)
  return useMemo(() => (ids.size ? entries.filter(entry => !ids.has(entry.id)) : entries), [entries, ids])
}

/** Settles once the list shows the save, so an edited row never flashes its old text. */
export function useUpdateDictionary() {
  return useMutation({
    mutationFn: async ({ term, replacement, id }: UpdateDictionaryArgs) => {
      const { rev } = await commands.updateDictionary(term, replacement, id ?? null)
      await mirrored(rev)
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
