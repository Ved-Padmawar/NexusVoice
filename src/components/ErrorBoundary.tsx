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
        <div role="alert" className="nv-empty h-full justify-center">
          <span className="nv-mark nv-mark--lg nv-mark--danger nv-empty__mark">
            <AlertTriangle size={20} strokeWidth={1.9} />
          </span>
          <p className="nv-empty__title">Something went wrong</p>
          <p className="nv-empty__desc">This section ran into an unexpected error. You can try loading it again.</p>
          <button type="button" onClick={this.reset} className="nv-btn nv-btn--secondary nv-btn--sm mt-3">
            <RotateCcw />
            Try again
          </button>
        </div>
      )
    }
    return this.props.children
  }
}
