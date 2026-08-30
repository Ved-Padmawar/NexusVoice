import type { ReactNode } from 'react'
import { AlertCircle, RefreshCw } from 'lucide-react'
import { useDelayedFlag } from '../lib/hooks'
import type { AsyncStatus } from '../store/asyncStatus'

interface Props {
  status: AsyncStatus
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

/**
 * Drives a section's loading / error / content UI from an {@link AsyncStatus}.
 * - `loading` → skeleton (only after `loaderDelayMs`, so fast loads don't flash)
 * - `error`   → inline message + Retry (re-runs the section's own fetch)
 * - else      → children
 */
export function SectionState({ status, error, onRetry, skeleton, children, loaderDelayMs = 250, hasData = false }: Props) {
  const showSkeleton = useDelayedFlag(status === 'loading', loaderDelayMs)

  if (status === 'error' && !hasData) {
    return (
      <div
        role="alert"
        className="flex flex-col items-center gap-3 py-12 px-6 text-center"
      >
        <div className="w-11 h-11 rounded-full bg-[color-mix(in_srgb,var(--danger)_12%,transparent)] text-destructive flex items-center justify-center">
          <AlertCircle size={20} strokeWidth={2} />
        </div>
        <p className="text-[13px] font-semibold text-(--fg-2) m-0">Couldn’t load this section</p>
        {error && <p className="text-[12px] text-muted-foreground max-w-72 leading-normal m-0">{error}</p>}
        <button
          type="button"
          onClick={onRetry}
          className="inline-flex items-center gap-1.5 h-8 px-3.5 rounded-(--r-md) border border-(--border) bg-(--surface) text-(--fg) text-[12px] font-medium cursor-pointer hover:border-(--accent) transition-colors duration-(--t-fast)"
        >
          <RefreshCw size={12} strokeWidth={2} />
          Try again
        </button>
      </div>
    )
  }

  if (status === 'loading' && !hasData) {
    return showSkeleton ? <>{skeleton}</> : null
  }

  return <>{children}</>
}
