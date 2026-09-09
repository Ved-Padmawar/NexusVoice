import { Component, type ReactNode, type ErrorInfo } from 'react'
import { RotateCcw } from 'lucide-react'
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
          className="flex h-full w-full flex-col items-center justify-center gap-2.5 p-6 text-center"
        >
          <p className="m-0 text-[14px] font-semibold text-(--fg)">Something went wrong</p>
          <p className="m-0 max-w-72 text-[12px] leading-[1.6] text-(--muted)">
            This section hit an unexpected error. Loading it again usually clears it.
          </p>
          <button type="button" onClick={this.reset} className="btn btn-quiet mt-1">
            <RotateCcw size={13} strokeWidth={1.9} />
            Try again
          </button>
        </div>
      )
    }
    return this.props.children
  }
}
