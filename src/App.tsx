import { useEffect, lazy, Suspense } from 'react'
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from './store/useAppStore'
import { Layout } from './components/Layout'
import { AuthGuard } from './components/AuthGuard'
import './App.css'

const Auth = lazy(() => import('./pages/Auth').then(m => ({ default: m.Auth })))
const Dashboard = lazy(() => import('./pages/Dashboard').then(m => ({ default: m.Dashboard })))
const Settings = lazy(() => import('./pages/Settings').then(m => ({ default: m.Settings })))

function App() {
  const { theme, init, isLoading, user } = useAppStore()

  useEffect(() => {
    init()
  }, [init])

  useEffect(() => {
    document.documentElement.dataset.theme = theme
  }, [theme])

  useEffect(() => {
    // Prevent window close — hide to tray instead
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
      <Suspense fallback={<div className="app-loading" role="status"><div className="loading-spinner" /></div>}>
        <Routes>
          <Route path="/" element={<Layout />}>
            {/* Protected routes */}
            <Route
              index
              element={
                <AuthGuard>
                  <Dashboard />
                </AuthGuard>
              }
            />
            <Route
              path="settings"
              element={
                <AuthGuard>
                  <Settings />
                </AuthGuard>
              }
            />

            {/* Auth route — redirects away if already logged in */}
            <Route
              path="auth"
              element={user ? <Navigate to="/" replace /> : <Auth />}
            />

            <Route path="*" element={<Navigate to="/" replace />} />
          </Route>
        </Routes>
      </Suspense>
    </BrowserRouter>
  )
}

export default App
