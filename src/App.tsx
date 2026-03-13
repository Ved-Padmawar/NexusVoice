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
  const { theme, isLoading, user, authChecking, activeRoute, listenForAuthReady, listenForModelEvents } = useAppStore()

  useEffect(() => {
    const cleanup = listenForAuthReady()
    return () => { cleanup.then(fn => fn()) }
  }, [listenForAuthReady])

  useEffect(() => {
    const cleanup = listenForModelEvents()
    return () => { cleanup.then(fn => fn()) }
  }, [listenForModelEvents])

  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused && useAppStore.getState().user) {
        useAppStore.getState().init()
      }
    })
    return () => { unlisten.then(fn => fn()) }
  }, [])

  useEffect(() => {
    const t = setTimeout(async () => {
      try {
        const update = await check()
        if (update?.available) {
          useAppStore.setState({ updateAvailable: update.version })
        }
      } catch { /* no network or no update endpoint */ }
    }, 3000)
    return () => clearTimeout(t)
  }, [])

  // Real-time transcript updates — prepend to store and refresh stats
  useEffect(() => {
    type NewTranscript = { id: number; content: string; createdAt: string }
    const unlisten = listen<NewTranscript>('transcript:new', (e) => {
      const t = e.payload
      useAppStore.setState(s => ({
        transcripts: [{ id: t.id, content: t.content, createdAt: t.createdAt }, ...s.transcripts],
      }))
      useAppStore.getState().fetchStats()
    })
    return () => { unlisten.then(fn => fn()) }
  }, [])

  useEffect(() => {
    document.documentElement.dataset.theme = theme
  }, [theme])

  useEffect(() => {
    const unlisten = getCurrentWindow().onCloseRequested(async (event) => {
      event.preventDefault()
      await getCurrentWindow().hide()
    })
    return () => { unlisten.then(fn => fn()) }
  }, [])

  if (authChecking || isLoading) {
    return (
      <div className="app-loading" role="status" aria-live="polite">
        <div className="loading-spinner" />
        <p className="loading-text">Loading…</p>
      </div>
    )
  }

  // Restore last active route; fall back to '/' if user is not logged in
  const initialRoute = user ? activeRoute : '/'

  return (
    <BrowserRouter>
      <Suspense fallback={<div className="app-loading" role="status"><div className="loading-spinner" /></div>}>
        <Routes>
          <Route path="/" element={<Layout />}>
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
            <Route
              path="auth"
              element={user ? <Navigate to="/" replace /> : <Auth />}
            />
            <Route path="*" element={<Navigate to={initialRoute} replace />} />
          </Route>
        </Routes>
      </Suspense>
    </BrowserRouter>
  )
}

export default App
