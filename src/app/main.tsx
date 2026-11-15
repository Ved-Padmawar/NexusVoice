import { attachConsole } from '@tauri-apps/plugin-log'
import '../index.css'
import App, { StartupLoader } from './App.tsx'
import { useAppStore } from './store/useAppStore'
import { showMainWindow } from './showMainWindow'
import { bootstrapMirror } from './mirrorBootstrap'

// Forward browser console to Tauri log file (no-op if plugin unavailable)
attachConsole().catch(() => {})

// Apply the persisted theme before React's first paint. App keeps it in sync.
document.documentElement.dataset.theme = useAppStore.getState().theme
// Slow storage or a rendering error must still leave a visible window.
setTimeout(showMainWindow, 1200)

bootstrapMirror(() => <App />, { loading: <StartupLoader /> })
