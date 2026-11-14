import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { attachConsole } from '@tauri-apps/plugin-log'
import { QueryClientProvider } from '@tanstack/react-query'
import '../index.css'
import App from './App.tsx'
import { ErrorBoundary } from './ErrorBoundary'
import { queryClient } from './queryClient'
import { useAppStore } from './store/useAppStore'
import { showMainWindow } from './showMainWindow'
import { followRust } from './sync'
import { logger } from '../shared/logger'

// Forward browser console to Tauri log file (no-op if plugin unavailable)
attachConsole().catch(() => {})

// Apply the persisted theme before React's first paint. App keeps it in sync.
document.documentElement.dataset.theme = useAppStore.getState().theme
// Slow storage or a rendering error must still leave a visible window.
setTimeout(showMainWindow, 1200)

const render = () => createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>
    </ErrorBoundary>
  </StrictMode>,
)

followRust().catch((e: unknown) => logger.error('could not follow backend changes', e)).finally(render)
