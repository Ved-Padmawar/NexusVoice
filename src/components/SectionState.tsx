import type { ReactNode } from 'react'
import { RefreshCw } from 'lucide-react'
import { useDelayedFlag } from '../lib/hooks'

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
      <div role="alert" className="flex flex-col items-center gap-2.5 px-6 py-14 text-center">
        <p className="m-0 text-[13px] font-semibold text-(--fg-2)">This section didn’t load</p>
        <p className="m-0 max-w-72 text-[12px] leading-[1.6] text-(--muted)">
          {error ?? 'Something went wrong reading your data.'}
        </p>
        <button type="button" onClick={onRetry} className="btn btn-sm btn-quiet mt-1">
          <RefreshCw size={12} strokeWidth={2} />
          Try again
        </button>
      </div>
    )
  }

  if (status === 'pending' && !hasData) {
    return showSkeleton ? <>{skeleton}</> : null
  }

  return <>{children}</>
}
