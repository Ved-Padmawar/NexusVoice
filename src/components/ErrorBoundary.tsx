import { Component, type ReactNode, type ErrorInfo } from 'react'
import { AlertTriangle, RotateCcw } from 'lucide-react'
import { logger } from '../lib/logger'

interface Props {
  children: ReactNode
  /** Custom fallback. Receives a `reset` fn to clear the error and re-render children. */
  fallback?: ReactNode | ((reset: () => void) => ReactNode)
  /** Called when the user retries — use to re-trigger a failed fetch, etc. */
  onReset?: () => void
}

interface State {
  hasError: boolean
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false }

  static getDerivedStateFromError(): State {
    return { hasError: true }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    logger.error(`ErrorBoundary caught: ${error.message}`, { stack: error.stack, componentStack: info.componentStack })
  }

  reset = () => {
    this.props.onReset?.()
    this.setState({ hasError: false })
  }

  render() {
    if (this.state.hasError) {
      const { fallback } = this.props
      if (typeof fallback === 'function') return fallback(this.reset)
      if (fallback !== undefined) return fallback
      return (
        <div
          role="alert"
          className="flex h-full w-full flex-col items-center justify-center gap-4 p-6 text-center"
        >
          <div className="flex size-12 items-center justify-center rounded-(--r-xl) bg-(--danger-soft) text-destructive">
            <AlertTriangle size={22} strokeWidth={1.75} />
          </div>
          <div className="flex flex-col gap-1">
            <p className="m-0 text-[14px] font-semibold text-(--fg)">Something went wrong</p>
            <p className="m-0 max-w-70 text-[12px] leading-relaxed text-muted-foreground">
              This section ran into an unexpected error. You can try loading it again.
            </p>
          </div>
          <button
            type="button"
            onClick={this.reset}
            className="inline-flex items-center gap-1.5 h-9 px-3.5 rounded-(--r-md) border border-(--border-soft) bg-(--surface) text-(--fg) text-[12px] font-medium cursor-pointer transition-[background,border-color] duration-(--t-fast) hover:bg-accent hover:border-(--border)"
          >
            <RotateCcw size={13} strokeWidth={1.75} />
            Try again
          </button>
        </div>
      )
    }
    return this.props.children
  }
}
