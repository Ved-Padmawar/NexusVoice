import type { ReactNode } from 'react'
import { AlertCircle, RefreshCw } from 'lucide-react'
import { useDelayedFlag } from '../lib/hooks'
import { Button } from './ui/button'

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
      <div role="alert" className="nv-empty col-span-full">
        <span className="nv-mark nv-mark--lg nv-mark--danger nv-empty__mark">
          <AlertCircle size={20} strokeWidth={2} />
        </span>
        <p className="nv-empty__title">Couldn’t load this section</p>
        {error && <p className="nv-empty__desc">{error}</p>}
        <Button variant="secondary" size="sm" className="mt-3" onClick={onRetry}>
          <RefreshCw />
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
