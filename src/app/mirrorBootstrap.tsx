import { StrictMode, type ReactNode } from 'react'
import { createRoot } from 'react-dom/client'
import { QueryClientProvider } from '@tanstack/react-query'
import { extractErrorMessage } from '../shared/errors'
import { logger } from '../shared/logger'
import { ErrorBoundary, ErrorScreen } from './ErrorBoundary'
import { mirrorRust, type SliceName } from './mirror'
import { queryClient } from './queryClient'

type Fallback = (message: string, retry: () => void) => ReactNode

/** Render a window once it mirrors the `slices` of Rust's state it shows; a
 *  failed subscribe shows `fallback`, whose retry subscribes again. */
export function bootstrapMirror(
  slices: SliceName[],
  render: () => ReactNode,
  { loading = null, fallback = (message, retry) => <ErrorScreen title="Couldn't connect" message={message} retry={retry} /> }: {
    loading?: ReactNode
    fallback?: Fallback
  } = {},
) {
  const root = createRoot(document.getElementById('root')!)

  const start = () => {
    root.render(loading)
    mirrorRust(slices).then(() => root.render(
      <StrictMode>
        <ErrorBoundary>
          <QueryClientProvider client={queryClient}>{render()}</QueryClientProvider>
        </ErrorBoundary>
      </StrictMode>,
    )).catch((error: unknown) => {
      logger.error('could not mirror the backend state', error)
      root.render(fallback(extractErrorMessage(error, 'This window could not sync with NexusVoice.'), start))
    })
  }

  start()
}
