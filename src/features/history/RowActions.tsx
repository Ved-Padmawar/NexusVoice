import { useRef, useState } from 'react'
import { ArrowsClockwiseIcon, CpuIcon, PushPinIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { extractErrorMessage } from '../../shared/errors'
import { cn } from '../../shared/utils'
import { MenuItem, Popover } from '../../shared/ui/Popover'
import { startRetranscribe, useDownloadedModels } from './jobs'
import { setPinned } from './queries'

export const ACTION_BUTTON = 'grid size-7 cursor-pointer place-items-center rounded-lg text-muted hover:bg-surface-hover hover:text-fg disabled:pointer-events-none disabled:opacity-40'

/** Re-transcribe an entry's audio with any installed model. Hidden while the
 *  entry has a job, whose row shows its progress instead. */
export function RetranscribeButton({ id, usedModel }: { id: number; usedModel: string | null }) {
  const anchorRef = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const models = useDownloadedModels(open)
  return (
    <>
      <button
        ref={anchorRef}
        type="button"
        aria-label="Re-transcribe"
        aria-expanded={open}
        data-tip="Re-transcribe"
        onClick={() => setOpen(o => !o)}
        className={cn(ACTION_BUTTON, 'aria-expanded:bg-surface-hover aria-expanded:text-fg')}
      >
        <ArrowsClockwiseIcon size={15} />
      </button>
      <Popover open={open} onOpenChange={setOpen} anchorRef={anchorRef} placement="bottom-end" className="w-64">
        <div className="px-3.5 pt-3 pb-1.5 text-2xs font-semibold tracking-wider text-muted uppercase">Re-transcribe with</div>
        {models.data?.map(model => (
          <MenuItem key={model.variant} onClick={() => { setOpen(false); void startRetranscribe(id, model.variant) }}>
            <CpuIcon size={15} className="text-muted" />
            <span className="truncate">{model.displayName}</span>
            {model.variant === usedModel
              ? <span className="ml-auto text-2xs font-semibold text-muted">Used</span>
              : model.isActive && <span className="ml-auto text-2xs font-semibold text-accent-ink">Active</span>}
          </MenuItem>
        ))}
        <p className="border-t border-line-soft px-3.5 py-2.5 text-xs text-muted">Replaces the text. You can undo it.</p>
      </Popover>
    </>
  )
}

export function PinButton({ id, pinned }: { id: number; pinned: boolean }) {
  return (
    <button
      type="button"
      aria-label={pinned ? 'Unpin' : 'Pin'}
      data-tip={pinned ? 'Unpin' : 'Pin: keep this audio for good'}
      data-tip-align="end"
      onClick={() => {
        setPinned(id, !pinned).catch((e: unknown) => toast.error(extractErrorMessage(e, 'Could not change the pin')))
      }}
      className={cn(ACTION_BUTTON, pinned && 'text-accent-ink')}
    >
      <PushPinIcon size={15} weight={pinned ? 'fill' : 'regular'} />
    </button>
  )
}
