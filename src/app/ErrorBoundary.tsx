import { Component, type ReactNode, type ErrorInfo } from 'react'
import { WarningIcon, ArrowCounterClockwiseIcon } from '@phosphor-icons/react'
import { logger } from '../shared/logger'
import { Button } from '../shared/ui/Button'

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

export function ErrorScreen({ title, message, retry }: { title: string; message: string; retry: () => void }) {
  return (
    <div role="alert" className="flex h-full w-full flex-col items-center justify-center gap-4 p-6 text-center">
      <span className="grid size-14 place-items-center rounded-2xl bg-danger-soft text-danger">
        <WarningIcon size={24} weight="duotone" />
      </span>
      <div className="flex flex-col gap-1">
        <p className="text-md font-semibold text-fg">{title}</p>
        <p className="max-w-70 text-sm text-balance text-muted">{message}</p>
      </div>
      <Button onClick={retry}>
        <ArrowCounterClockwiseIcon size={13} weight="bold" />
        Try again
      </Button>
    </div>
  )
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
        <ErrorScreen title="Something went wrong" message="This section ran into an unexpected error. You can try loading it again." retry={this.reset} />
      )
    }
    return this.props.children
  }
}
