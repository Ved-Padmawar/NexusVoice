import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { attachConsole } from '@tauri-apps/plugin-log'
import { restoreStateCurrent, StateFlags } from '@tauri-apps/plugin-window-state'
import { getCurrentWindow } from '@tauri-apps/api/window'
import './index.css'
import App from './App.tsx'
import { ErrorBoundary } from './components/ErrorBoundary'

// Forward browser console to Tauri log file (no-op if plugin unavailable)
attachConsole().catch(() => {})

// Restore persisted window position/size, then show — prevents flash and taskbar overlap
restoreStateCurrent(StateFlags.ALL)
  .catch(() => {})
  .finally(() => getCurrentWindow().show().catch(() => {}))

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </StrictMode>,
)
