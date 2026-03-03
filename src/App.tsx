import { useEffect } from 'react'
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from './store/useAppStore'
import { Layout } from './components/Layout'
import { Auth } from './pages/Auth'
import { Dashboard } from './pages/Dashboard'
import { Settings } from './pages/Settings'
import './App.css'

function App() {
  const { theme, init, isLoading } = useAppStore()

  useEffect(() => {
    init()
  }, [init])

  useEffect(() => {
    document.documentElement.dataset.theme = theme
  }, [theme])

  useEffect(() => {
    // Prevent window close, hide instead
    const unlisten = getCurrentWindow().onCloseRequested(async (event) => {
      event.preventDefault()
      await getCurrentWindow().hide()
    })

    return () => {
      unlisten.then(fn => fn())
    }
  }, [])

  if (isLoading) {
    return (
      <div className="app-loading" role="status" aria-live="polite">
        <div className="loading-spinner" />
        <p className="loading-text">Loading…</p>
      </div>
    )
  }

  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Layout />}>
          <Route index element={<Dashboard />} />
          <Route path="auth" element={<Auth />} />
          <Route path="settings" element={<Settings />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </BrowserRouter>
  )
}

export default App
