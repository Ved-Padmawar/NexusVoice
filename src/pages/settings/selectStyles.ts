/** Shared chrome for the Radix selects in Settings, so the microphone and
 *  language pickers stay identical without duplicating class strings. */

export const SELECT_TRIGGER =
  'field group relative flex cursor-pointer items-center pl-8 pr-8 text-left ' +
  'data-[state=open]:shadow-[inset_0_0_0_1px_var(--accent)] ' +
  'disabled:opacity-50'

/** `rounded-(--r-md)` overrides `.pop`'s larger radius: a list reads better
 *  with a modest corner than a pill, and the rows run to the panel edge. */
export const SELECT_CONTENT =
  'pop z-50 w-(--radix-select-trigger-width) overflow-hidden rounded-(--r-md) ' +
  'data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 ' +
  'data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95'

/** No radius and no side inset: the highlight spans the full width and butts
 *  against the top and bottom edges, which the container clips into shape. */
export const SELECT_ITEM =
  'flex h-8 cursor-pointer select-none items-center px-3 text-[12px] text-(--fg-2) outline-none ' +
  'data-highlighted:bg-(--surface-hover) data-highlighted:text-(--fg) ' +
  'data-[state=checked]:font-semibold data-[state=checked]:text-(--on-soft)'
