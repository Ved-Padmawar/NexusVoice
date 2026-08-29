import { useEffect, lazy, Suspense, type ReactNode } from 'react'
import { BrowserRouter, Routes, Route, Navigate, useLocation } from 'react-router'
import { motion } from 'framer-motion'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Toaster } from 'sonner'

import { useAppStore, type Transcript } from './store/useAppStore'
import { EVENTS } from './lib/events'
import { ROUTES } from './lib/routes'
import { useEventListener, useDelayedFlag } from './lib/hooks'
import { Layout } from './components/Layout'
import { AuthGuard } from './components/AuthGuard'
import { ErrorBoundary } from './components/ErrorBoundary'
import { ModelPickerModal } from './components/ModelPickerModal'

const Spinner = () => (
  <motion.div className="w-7 h-7 rounded-full border-2 border-[var(--border)] border-t-[var(--accent)]" animate={{ rotate: 360 }} transition={{ duration: 0.65, ease: 'linear', repeat: Infinity }} />
)

/** Full-screen startup loader. Spinner appears only after a short delay so a fast
 * auth check doesn't flash it; the themed background shows immediately. */
function StartupLoader() {
  const showSpinner = useDelayedFlag(true, 250)
  return (
    <div className="flex flex-col items-center justify-center min-h-dvh gap-[14px] bg-[var(--bg)]" role="status" aria-live="polite" data-tauri-drag-region>
      {showSpinner && (
        <>
          <Spinner />
          <p className="text-[12px] text-[var(--muted)]">Loading…</p>
        </>
      )}
    </div>
  )
}

/** Suspense fallback for lazy routes — delayed so quick chunk loads don't flash. */
function RouteFallback() {
  const showSpinner = useDelayedFlag(true, 250)
  return (
    <div className="flex items-center justify-center min-h-dvh bg-[var(--bg)]" role="status" data-tauri-drag-region>
      {showSpinner && <Spinner />}
    </div>
  )
}

const Auth = lazy(() => import('./pages/Auth').then(m => ({ default: m.Auth })))
const Dashboard = lazy(() => import('./pages/Dashboard').then(m => ({ default: m.Dashboard })))
const Settings = lazy(() => import('./pages/Settings').then(m => ({ default: m.Settings })))
const Dictionary = lazy(() => import('./pages/Dictionary').then(m => ({ default: m.Dictionary })))

function App() {
  const { theme, user, authChecking, activeRoute, modelChosen, listenForAuthReady, listenForModelEvents } = useAppStore()

  useEffect(() => {
    const cleanup = listenForAuthReady()
    return () => { cleanup.then(fn => fn()).catch(() => {}) }
  }, [listenForAuthReady])

  useEffect(() => {
    const cleanup = listenForModelEvents()
    return () => { cleanup.then(fn => fn()).catch(() => {}) }
  }, [listenForModelEvents])

  useEffect(() => {
    const t = setTimeout(() => {
      useAppStore.getState().checkForUpdate().catch(() => {})
    }, 3000)
    return () => clearTimeout(t)
  }, [])

  useEventListener<Transcript>(EVENTS.TRANSCRIPT_NEW, (t) => {
    useAppStore.setState(s => ({ transcripts: [t, ...s.transcripts] }))
    useAppStore.getState().loadStats()
  })

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

  if (authChecking) {
    return <StartupLoader />
  }

  const initialRoute = user ? activeRoute : '/'

  return (
    <BrowserRouter>
      <Suspense fallback={<RouteFallback />}>
        <AnimatedRoutes initialRoute={initialRoute} user={user} />
      </Suspense>
      {user && !modelChosen && <ModelPickerModal />}
      <Toaster
        position="bottom-right"
        toastOptions={{
          style: {
            background: 'var(--panel)',
            border: '1px solid var(--border)',
            color: 'var(--fg)',
            fontSize: '13px',
            borderRadius: 'var(--r-lg)',
            boxShadow: 'var(--shadow-md)',
          },
        }}
      />
    </BrowserRouter>
  )
}

const pageVariants = {
  initial: { opacity: 0, y: 6 },
  animate: { opacity: 1, y: 0 },
  exit:    { opacity: 0, y: -6 },
}
const pageTransition = { duration: 0.18, ease: 'easeInOut' as const }

function PageTransition({ id, children }: { id: string; children: ReactNode }) {
  return (
    <motion.div key={id} className="contents" variants={pageVariants} initial="initial" animate="animate" exit="exit" transition={pageTransition}>
      {children}
    </motion.div>
  )
}

function AnimatedRoutes({ initialRoute, user }: { initialRoute: string; user: { id: number; email: string } | null }) {
  const location = useLocation()
  return (
    <Routes location={location}>
      <Route path={ROUTES.DASHBOARD} element={<Layout />}>
        <Route index element={
          <AuthGuard>
            <ErrorBoundary>
              <PageTransition id="dashboard"><Dashboard /></PageTransition>
            </ErrorBoundary>
          </AuthGuard>
        } />
        <Route path={ROUTES.SETTINGS.slice(1)} element={
          <AuthGuard>
            <ErrorBoundary>
              <PageTransition id="settings"><Settings /></PageTransition>
            </ErrorBoundary>
          </AuthGuard>
        } />
        <Route path={ROUTES.DICTIONARY.slice(1)} element={
          <AuthGuard>
            <ErrorBoundary>
              <PageTransition id="dictionary"><Dictionary /></PageTransition>
            </ErrorBoundary>
          </AuthGuard>
        } />
        <Route path={ROUTES.AUTH.slice(1)} element={user ? <Navigate to={ROUTES.DASHBOARD} replace /> : <Auth />} />
        <Route path="*" element={<Navigate to={initialRoute} replace />} />
      </Route>
    </Routes>
  )
}

export default App
