import { toast } from 'sonner'
import { create } from 'zustand'

const UNDO_WINDOW_MS = 5000

/** Ids waiting out an Undo toast. The query cache keeps their rows: hiding
 *  them at read time means a refetch inside the window cannot bring them back. */
export function createPendingDeletes() {
  const store = create<{ ids: ReadonlySet<number> }>(() => ({ ids: new Set() }))
  const toggle = (id: number, pending: boolean) => store.setState(({ ids }) => {
    const next = new Set(ids)
    if (pending) next.add(id)
    else next.delete(id)
    return { ids: next }
  })
  return {
    useIds: () => store(state => state.ids),
    /** For reads outside React, such as an export. */
    current: () => store.getState().ids,
    hide: (id: number) => toggle(id, true),
    show: (id: number) => toggle(id, false),
  }
}

export type PendingDeletes = ReturnType<typeof createPendingDeletes>

type DeleteWithUndo = {
  message: string
  id: number
  pending: PendingDeletes
  /** The real delete, run once the undo window closes. Resolves once the
   *  cache no longer holds the row. */
  commit: () => Promise<unknown>
}

/** The undo of each delete still inside its window, newest last. */
const undoable: (() => void)[] = []
let listening = false

const isTextField = (target: EventTarget | null) =>
  target instanceof HTMLElement && (target.isContentEditable || target.matches('input, textarea, select'))

/** Ctrl+Z undoes the newest delete, unless a text field is focused: its own undo comes first. */
function onUndoKey(e: KeyboardEvent) {
  if (!(e.ctrlKey || e.metaKey) || e.shiftKey || e.altKey || e.key.toLowerCase() !== 'z') return
  if (isTextField(e.target)) return
  const undo = undoable.at(-1)
  if (!undo) return
  e.preventDefault()
  undo()
}

/**
 * Hides an item now and deletes it only when the undo toast closes. Quitting
 * inside the window keeps the item, which is the safe way to fail.
 */
export function deleteWithUndo({ message, id, pending, commit }: DeleteWithUndo) {
  pending.hide(id)
  const show = () => pending.show(id)
  let settled = false
  const settle = () => {
    if (settled) return false
    settled = true
    undoable.splice(undoable.indexOf(undo), 1)
    return true
  }
  const finish = () => {
    // A failed delete is reported by its mutation; showing the row puts it back.
    if (settle()) commit().then(show, show)
  }
  const restore = () => {
    if (settle()) show()
  }
  const undo = () => {
    restore()
    toast.dismiss(toastId)
  }
  const toastId = toast(message, {
    duration: UNDO_WINDOW_MS,
    action: { label: 'Undo', onClick: restore },
    onAutoClose: finish,
    onDismiss: finish,
  })
  undoable.push(undo)
  if (!listening) {
    listening = true
    window.addEventListener('keydown', onUndoKey)
  }
}
