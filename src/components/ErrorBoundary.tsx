import { Component, type ReactNode, type ErrorInfo } from 'react'
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
          style={{
            padding: '24px',
            textAlign: 'center',
            color: 'var(--fg-2)',
            fontFamily: 'inherit',
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            gap: '12px',
          }}
        >
          <p style={{ margin: 0, color: 'var(--danger)', fontSize: '13px', fontWeight: 600 }}>
            Something went wrong.
          </p>
          <button
            type="button"
            onClick={this.reset}
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: '6px',
              height: '32px',
              padding: '0 14px',
              borderRadius: 'var(--r-md)',
              border: '1px solid var(--border)',
              background: 'var(--surface)',
              color: 'var(--fg)',
              fontSize: '12px',
              fontWeight: 500,
              cursor: 'pointer',
            }}
          >
            Try again
          </button>
        </div>
      )
    }
    return this.props.children
  }
}
