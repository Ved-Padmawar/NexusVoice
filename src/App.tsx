import { useEffect, lazy, Suspense } from 'react'
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { listen } from '@tauri-apps/api/event'
import { check } from '@tauri-apps/plugin-updater'
import { useAppStore } from './store/useAppStore'
import { Layout } from './components/Layout'
import { AuthGuard } from './components/AuthGuard'
import './App.css'

const Auth = lazy(() => import('./pages/Auth').then(m => ({ default: m.Auth })))
const Dashboard = lazy(() => import('./pages/Dashboard').then(m => ({ default: m.Dashboard })))
const Settings = lazy(() => import('./pages/Settings').then(m => ({ default: m.Settings })))
const Dictionary = lazy(() => import('./pages/Dictionary').then(m => ({ default: m.Dictionary })))

function App() {
  const { theme, isLoading, user, authChecking, listenForAuthReady, listenForModelEvents } = useAppStore()

  useEffect(() => {
    // Subscribe to auth:ready / auth:unauthenticated before anything else
    // init() is called inside listenForAuthReady on auth:ready — not here
    const cleanup = listenForAuthReady()
    return () => { cleanup.then(fn => fn()) }
  }, [listenForAuthReady])

  useEffect(() => {
    const cleanup = listenForModelEvents()
    return () => { cleanup.then(fn => fn()) }
  }, [listenForModelEvents])

  // Refresh all data when window regains focus (covers tray re-open, alt-tab back)
  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused && useAppStore.getState().user) {
        useAppStore.getState().init()
      }
    })
    return () => { unlisten.then(fn => fn()) }
  }, [])

  // Silent update check on startup — runs once after app loads
  useEffect(() => {
    const run = async () => {
      try {
        const update = await check()
        if (update?.available) {
          useAppStore.setState({ updateAvailable: update.version })
        }
      } catch { /* ignore — no network or no update endpoint yet */ }
    }
    // Delay slightly so app startup isn't blocked
    const t = setTimeout(run, 3000)
    return () => clearTimeout(t)
  }, [])

  // Real-time transcript updates — prepend without full refresh
  useEffect(() => {
    type NewTranscript = { id: number; content: string; createdAt: string }
    const unlisten = listen<NewTranscript>('transcript:new', (e) => {
      const t = e.payload
      useAppStore.setState(s => ({
        transcripts: [{ id: t.id, content: t.content, createdAt: t.createdAt }, ...s.transcripts],
      }))
      // Also refresh stats (word count etc changed)
      useAppStore.getState().fetchStats()
    })
    return () => { unlisten.then(fn => fn()) }
  }, [])

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

  // Wait for backend to emit auth:ready or auth:unauthenticated before rendering routes
  if (authChecking || isLoading) {
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
            <Route
              path="dictionary"
              element={
                <AuthGuard>
                  <Dictionary />
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
