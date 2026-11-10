import { useState, useEffect, useCallback } from 'react'
import { TrashIcon, XIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { formatModelSize } from './models'
import { VENDORS, vendorForFamily, isRaster } from '../../shared/vendors'
import { NO_MODELS, useCatalog } from './queries'
import { Modal } from '../../shared/ui/Modal'
import { VendorMark } from '../../shared/ui/VendorMark'
import type { DownloadedModel } from '../../shared/types'

type Props = {
  open: boolean
  onClose: () => void
}

/** Models on this PC: where deleting lives. The total leads; the list, largest first, is the breakdown. */
export function ModelManagerModal({ open, onClose }: Props) {
  const [models, setModels] = useState<DownloadedModel[]>([])
  // A Set: deleting A then B must not re-enable A while it is still going.
  const [deleting, setDeleting] = useState<ReadonlySet<string>>(new Set())
  const [confirming, setConfirming] = useState<string | null>(null)
  const catalog = useCatalog().data ?? NO_MODELS

  const load = useCallback(() => {
    commands.getDownloadedModels()
      .then(setModels)
      .catch(() => {})
  }, [])

  useEffect(() => { if (open) load() }, [open, load])

  const handleDelete = async (variant: string, displayName: string) => {
    setConfirming(null)
    setDeleting(s => new Set(s).add(variant))
    try {
      await commands.deleteModel(variant)
      toast.success(`${displayName} deleted`)
      load()
    } catch (e) {
      toast.error(extractErrorMessage(e, 'Failed to delete model'))
    } finally {
      setDeleting(s => { const next = new Set(s); next.delete(variant); return next })
    }
  }

  const rows = [...models].sort((a, b) => b.sizeBytes - a.sizeBytes)
  const [total, unit] = formatModelSize(rows.reduce((acc, m) => acc + m.sizeBytes, 0)).split(' ')

  return (
    <Modal open={open} onClose={onClose} aria-labelledby="manager-title" className="max-h-160 w-115">
      <div className="flex shrink-0 items-start gap-3 px-6 pt-5 pb-3">
        <div className="flex-1">
          <h2 id="manager-title" className="text-sm font-semibold text-fg-2">Models on this PC</h2>
          <p className="mt-1 flex items-baseline gap-1.5 tabular-nums">
            <span className="text-[30px] leading-none font-bold tracking-[-0.03em] text-fg">{total}</span>
            <span className="text-md font-semibold text-muted">{unit}</span>
            <span className="ml-1.5 text-xs text-muted">
              {rows.length > 0 ? `across ${rows.length} model${rows.length === 1 ? '' : 's'}` : 'No models downloaded'}
            </span>
          </p>
        </div>
        <button
          type="button"
          aria-label="Close"
          onClick={onClose}
          className="grid size-6 cursor-pointer place-items-center rounded-md text-muted hover:bg-danger-soft hover:text-danger"
        >
          <XIcon size={12} weight="bold" />
        </button>
      </div>

      <ul className="min-h-0 flex-1 overflow-y-auto overscroll-none px-3 pb-2">
        {rows.map((model, i) => {
          const family = catalog.find(m => m.id === model.variant)?.family
          const vendor = family ? vendorForFamily(family) : null
          const color = vendor && !isRaster(vendor) ? VENDORS[vendor].color : null
          return (
            <li key={model.variant}>
              {i > 0 && <div aria-hidden="true" className="mx-3 h-px bg-line-soft" />}
              <div className="flex h-14 items-center gap-3 rounded-lg px-3 hover:bg-surface-hover has-[[data-del]:hover]:bg-transparent">
                <span
                  className="grid size-8 shrink-0 place-items-center rounded-lg bg-field"
                  style={color ? { background: `color-mix(in oklab, ${color} 14%, transparent)` } : undefined}
                >
                  {vendor && <VendorMark vendor={vendor} className="size-4.5" />}
                </span>
                <span className="flex min-w-0 flex-1 items-center gap-2 text-sm font-semibold text-fg">
                  <span className="truncate">{model.displayName}</span>
                  {model.isActive && (
                    <span className="shrink-0 rounded-md bg-accent-soft px-1.5 py-px text-[10px] font-semibold text-accent-ink">In use</span>
                  )}
                </span>
                <span className="text-sm font-medium text-fg-2 tabular-nums">{formatModelSize(model.sizeBytes)}</span>
                {/* Fixed width: asking to confirm doesn't move the size beside it. */}
                <span className="flex w-17 shrink-0 justify-end">
                  {confirming === model.variant ? (
                    <button
                      type="button"
                      data-del
                      autoFocus
                      aria-label={`Confirm deleting ${model.displayName}`}
                      onClick={() => void handleDelete(model.variant, model.displayName)}
                      onBlur={() => setConfirming(null)}
                      onKeyDown={e => { if (e.key === 'Escape') { e.stopPropagation(); setConfirming(null) } }}
                      className="h-8 w-full cursor-pointer rounded-lg bg-danger-soft text-xs font-semibold text-danger ring-1 ring-inset ring-danger/40 transition-shadow hover:ring-danger"
                    >
                      Delete
                    </button>
                  ) : (
                    <button
                      type="button"
                      data-del
                      aria-label={`Delete ${model.displayName}`}
                      disabled={deleting.has(model.variant)}
                      onClick={() => setConfirming(model.variant)}
                      className="grid size-8 cursor-pointer place-items-center rounded-lg text-muted transition-[color,background-color] hover:bg-danger-soft hover:text-danger disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      <TrashIcon size={14} />
                    </button>
                  )}
                </span>
              </div>
            </li>
          )
        })}
      </ul>

      <p className="shrink-0 border-t border-line-soft px-6 pt-3 pb-4 text-2xs text-pretty text-muted">
        Every NexusVoice build on this PC shares these files. Deleting the active model frees its memory; transcription will prompt you to pick a model again.
      </p>
    </Modal>
  )
}
