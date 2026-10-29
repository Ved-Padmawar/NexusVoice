import type { ReactElement } from 'react'
import { QueryClientProvider } from '@tanstack/react-query'
import { render } from '@testing-library/react'
import { queryClient } from '../app/queryClient'

/** Render against the app's own client, which event handlers also write to. */
export function renderWithQuery(ui: ReactElement) {
  queryClient.clear()
  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>)
}
