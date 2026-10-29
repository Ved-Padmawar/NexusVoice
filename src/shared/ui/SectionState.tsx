import type { ReactNode } from 'react'
import { WarningCircleIcon, ArrowsClockwiseIcon } from '@phosphor-icons/react'
import { useDelayedFlag } from '../hooks'
import { Button } from './Button'

type SectionStatus = 'pending' | 'error' | 'success'

interface Props {
  status: SectionStatus
  error?: string | null
  /** Re-run the section's own fetch. */
  onRetry: () => void
  /** Shown (after a short delay) while loading. */
  skeleton: ReactNode
  /** Shown on success. */
  children: ReactNode
  /** Delay before the skeleton appears, to avoid flashing on fast loads. */
  loaderDelayMs?: number
  /** Content is already on screen: a reload updates it in place instead of
   * unmounting it for the skeleton. */
  hasData?: boolean
}

export function SectionState({ status, error, onRetry, skeleton, children, loaderDelayMs = 250, hasData = false }: Props) {
  const showSkeleton = useDelayedFlag(status === 'pending', loaderDelayMs)

  if (status === 'error' && !hasData) {
    return (
      <div
        role="alert"
        className="flex flex-col items-center gap-3 py-12 px-6 text-center"
      >
        <span className="grid size-11 place-items-center rounded-full bg-danger-soft text-danger">
          <WarningCircleIcon size={20} weight="bold" />
        </span>
        <p className="text-base font-semibold text-fg-2">Couldn’t load this section</p>
        {error && <p className="max-w-72 text-sm text-muted">{error}</p>}
        <Button onClick={onRetry}>
          <ArrowsClockwiseIcon size={12} weight="bold" />
          Try again
        </Button>
      </div>
    )
  }

  if (status === 'pending' && !hasData) {
    return showSkeleton ? <>{skeleton}</> : null
  }

  return <>{children}</>
}
