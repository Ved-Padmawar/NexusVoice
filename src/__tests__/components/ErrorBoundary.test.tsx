/**
 * The crash net. If this stops catching, a render error in any section blanks
 * the whole window instead of one panel.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, cleanup, fireEvent } from '@testing-library/react'
import { ErrorBoundary } from '../../components/ErrorBoundary'
import { logger } from '../../lib/logger'

vi.mock('../../lib/logger', () => ({ logger: { error: vi.fn() } }))

function Boom({ throws }: { throws: boolean }) {
  if (throws) throw new Error('kaboom')
  return <p>recovered content</p>
}

let consoleError: ReturnType<typeof vi.spyOn>
beforeEach(() => {
  // React logs caught render errors; silence it so the run stays readable.
  consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
  vi.clearAllMocks()
})
afterEach(() => { consoleError.mockRestore(); cleanup() })

describe('ErrorBoundary', () => {
  it('renders its children when nothing throws', () => {
    render(<ErrorBoundary><p>all good</p></ErrorBoundary>)
    expect(screen.getByText('all good')).toBeInTheDocument()
  })

  it('shows a recoverable fallback instead of propagating the crash', () => {
    render(<ErrorBoundary><Boom throws /></ErrorBoundary>)
    expect(screen.getByRole('alert')).toBeInTheDocument()
    expect(screen.getByText('Something went wrong')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /try again/i })).toBeInTheDocument()
  })

  it('logs the error so a user crash is diagnosable from the log file', () => {
    // The user can only send the log; a silent boundary makes the bug invisible.
    render(<ErrorBoundary><Boom throws /></ErrorBoundary>)
    expect(logger.error).toHaveBeenCalledOnce()
    expect(vi.mocked(logger.error).mock.calls[0][0]).toContain('kaboom')
  })

  it('re-renders the children after the user retries', () => {
    // Reset must actually clear the error state, or "Try again" is a dead button.
    // React re-invokes a failing render, so the throw is driven by an explicit
    // flag rather than an attempt counter.
    let shouldThrow = true
    const Flaky = () => <Boom throws={shouldThrow} />

    render(<ErrorBoundary><Flaky /></ErrorBoundary>)
    expect(screen.getByRole('alert')).toBeInTheDocument()

    shouldThrow = false
    fireEvent.click(screen.getByRole('button', { name: /try again/i }))
    expect(screen.getByText('recovered content')).toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('calls onReset before clearing the error, so a refetch can be queued', async () => {
    const onReset = vi.fn()
    render(<ErrorBoundary onReset={onReset}><Boom throws /></ErrorBoundary>)
    fireEvent.click(screen.getByRole('button', { name: /try again/i }))
    expect(onReset).toHaveBeenCalledOnce()
  })

  it('renders a custom element fallback in place of the default', () => {
    render(<ErrorBoundary fallback={<p>custom failure</p>}><Boom throws /></ErrorBoundary>)
    expect(screen.getByText('custom failure')).toBeInTheDocument()
    expect(screen.queryByText('Something went wrong')).not.toBeInTheDocument()
  })

  it('hands a render-prop fallback a working reset', () => {
    let shouldThrow = true
    const Flaky = () => <Boom throws={shouldThrow} />
    render(
      <ErrorBoundary fallback={reset => <button onClick={reset}>retry now</button>}>
        <Flaky />
      </ErrorBoundary>,
    )
    expect(screen.getByRole('button', { name: 'retry now' })).toBeInTheDocument()

    shouldThrow = false
    fireEvent.click(screen.getByRole('button', { name: 'retry now' }))
    expect(screen.getByText('recovered content')).toBeInTheDocument()
  })
})
