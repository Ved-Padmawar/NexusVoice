import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { attachConsole } from '@tauri-apps/plugin-log'
import { getCurrentWindow } from '@tauri-apps/api/window'
import './index.css'
import App from './App.tsx'
import { ErrorBoundary } from './components/ErrorBoundary'

// Forward browser console to Tauri log file (no-op if plugin unavailable)
attachConsole().catch(() => {})

// Show main window once JS is ready
getCurrentWindow().show().catch(() => {})

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </StrictMode>,
)
