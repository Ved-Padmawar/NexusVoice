import { ArrowsClockwiseIcon, PushPinIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { extractErrorMessage } from '../../shared/errors'
import { cn } from '../../shared/utils'
import { startRetranscribe } from './jobs'
import { setPinned } from './queries'

export const ACTION_BUTTON = 'grid size-7 cursor-pointer place-items-center rounded-lg text-muted hover:bg-surface-hover hover:text-fg disabled:pointer-events-none disabled:opacity-40'

/** Re-transcribe an entry's audio with the active model. Hidden while the
 *  entry has a job, whose row shows its progress instead. */
export function RetranscribeButton({ id }: { id: number }) {
  return (
    <button
      type="button"
      aria-label="Re-transcribe"
      data-tip="Re-transcribe with the active model"
      onClick={() => void startRetranscribe(id)}
      className={ACTION_BUTTON}
    >
      <ArrowsClockwiseIcon size={15} />
    </button>
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
