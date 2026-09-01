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
import { ErrorBoundary } from './components/ErrorBoundary'

const Spinner = () => (
  <motion.div className="w-7 h-7 rounded-full border-2 border-(--border) border-t-(--accent)" animate={{ rotate: 360 }} transition={{ duration: 0.65, ease: 'linear', repeat: Infinity }} />
)

/** Full-screen startup loader. Spinner appears only after a short delay so a fast
 * database open doesn't flash it; the themed background shows immediately. */
function StartupLoader() {
  const showSpinner = useDelayedFlag(true, 250)
  return (
    <div className="flex flex-col items-center justify-center min-h-dvh gap-3.5 bg-background" role="status" aria-live="polite" data-tauri-drag-region>
      {showSpinner && (
        <>
          <Spinner />
          <p className="text-[12px] text-muted-foreground">Loading…</p>
        </>
      )}
    </div>
  )
}

/** Suspense fallback for lazy routes — delayed so quick chunk loads don't flash. */
function RouteFallback() {
  const showSpinner = useDelayedFlag(true, 250)
  return (
    <div className="flex items-center justify-center min-h-dvh bg-background" role="status" data-tauri-drag-region>
      {showSpinner && <Spinner />}
    </div>
  )
}

const Dashboard = lazy(() => import('./pages/Dashboard').then(m => ({ default: m.Dashboard })))
const Settings = lazy(() => import('./pages/Settings').then(m => ({ default: m.Settings })))
const Dictionary = lazy(() => import('./pages/Dictionary').then(m => ({ default: m.Dictionary })))
// Shown once, on first run. Eager, it put its whole tree on every startup.
const ModelPickerModal = lazy(() => import('./components/ModelPickerModal').then(m => ({ default: m.ModelPickerModal })))

function App() {
  const { theme, starting, activeRoute, modelChosen, startup, listenForModelEvents } = useAppStore()

  useEffect(() => {
    startup()
  }, [startup])

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

  if (starting) {
    return <StartupLoader />
  }

  return (
    <BrowserRouter>
      <Suspense fallback={<RouteFallback />}>
        <AnimatedRoutes initialRoute={activeRoute} />
      </Suspense>
      {/* No fallback: nothing should flash on screen while the chunk loads. */}
      {!modelChosen && (
        <Suspense fallback={null}>
          <ModelPickerModal />
        </Suspense>
      )}
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

function AnimatedRoutes({ initialRoute }: { initialRoute: string }) {
  const location = useLocation()
  return (
    <Routes location={location}>
      <Route path={ROUTES.DASHBOARD} element={<Layout />}>
        <Route index element={
          <ErrorBoundary>
            <PageTransition id="dashboard"><Dashboard /></PageTransition>
          </ErrorBoundary>
        } />
        <Route path={ROUTES.SETTINGS.slice(1)} element={
          <ErrorBoundary>
            <PageTransition id="settings"><Settings /></PageTransition>
          </ErrorBoundary>
        } />
        <Route path={ROUTES.DICTIONARY.slice(1)} element={
          <ErrorBoundary>
            <PageTransition id="dictionary"><Dictionary /></PageTransition>
          </ErrorBoundary>
        } />
        <Route path="*" element={<Navigate to={initialRoute} replace />} />
      </Route>
    </Routes>
  )
}

export default App
