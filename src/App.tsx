import { useEffect, lazy, Suspense } from 'react'
import { BrowserRouter, Routes, Route, Navigate, useLocation } from 'react-router-dom'
import { motion } from 'framer-motion'
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

  // Real-time dictionary updates — merge auto-learned words into store
  useEffect(() => {
    type DictEntry = { id: number; term: string; replacement: string; createdAt: string }
    const unlisten = listen<DictEntry[]>('dictionary:updated', (e) => {
      useAppStore.setState(s => {
        const existing = new Set(s.dictionary.map(d => d.term))
        const newEntries = e.payload.filter(d => !existing.has(d.term))
        return newEntries.length > 0
          ? { dictionary: [...s.dictionary, ...newEntries] }
          : {}
      })
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
      <div className="app-loading" role="status" aria-live="polite" data-tauri-drag-region>
        <div className="loading-spinner" />
        <p className="loading-text">Loading…</p>
      </div>
    )
  }

  // Restore last active route; fall back to '/' if user is not logged in
  const initialRoute = user ? activeRoute : '/'

  return (
    <BrowserRouter>
      <Suspense fallback={<div className="app-loading" role="status" data-tauri-drag-region><div className="loading-spinner" /></div>}>
        <AnimatedRoutes initialRoute={initialRoute} user={user} />
      </Suspense>
    </BrowserRouter>
  )
}

const pageVariants = {
  initial: { opacity: 0, y: 6 },
  animate: { opacity: 1, y: 0 },
  exit:    { opacity: 0, y: -6 },
}
const pageTransition = { duration: 0.18, ease: 'easeInOut' as const }

function AnimatedRoutes({ initialRoute, user }: { initialRoute: string; user: { id: number; email: string } | null }) {
  const location = useLocation()
  return (
    <Routes location={location}>
      <Route path="/" element={<Layout />}>
        <Route index element={
          <AuthGuard>
            <motion.div key="dashboard" className="page-motion" variants={pageVariants} initial="initial" animate="animate" exit="exit" transition={pageTransition}>
              <Dashboard />
            </motion.div>
          </AuthGuard>
        } />
        <Route path="settings" element={
          <AuthGuard>
            <motion.div key="settings" className="page-motion" variants={pageVariants} initial="initial" animate="animate" exit="exit" transition={pageTransition}>
              <Settings />
            </motion.div>
          </AuthGuard>
        } />
        <Route path="dictionary" element={
          <AuthGuard>
            <motion.div key="dictionary" className="page-motion" variants={pageVariants} initial="initial" animate="animate" exit="exit" transition={pageTransition}>
              <Dictionary />
            </motion.div>
          </AuthGuard>
        } />
        <Route path="auth" element={user ? <Navigate to="/" replace /> : <Auth />} />
        <Route path="*" element={<Navigate to={initialRoute} replace />} />
      </Route>
    </Routes>
  )
}

export default App
