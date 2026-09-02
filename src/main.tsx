import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { attachConsole } from '@tauri-apps/plugin-log'
import './index.css'
import App from './App.tsx'
import { ErrorBoundary } from './components/ErrorBoundary'
import { useAppStore } from './store/useAppStore'
import { showMainWindow } from './lib/showMainWindow'

// Forward browser console to Tauri log file (no-op if plugin unavailable)
attachConsole().catch(() => {})

// Apply the persisted theme before React's first paint. App keeps it in sync.
document.documentElement.dataset.theme = useAppStore.getState().theme
// Slow storage or a rendering error must still leave a visible window.
setTimeout(showMainWindow, 1200)

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </StrictMode>,
)
