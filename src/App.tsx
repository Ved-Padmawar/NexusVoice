import { useShallow } from 'zustand/react/shallow'
import { useEffect, useState, lazy, Suspense } from 'react'
import { BrowserRouter, Routes, Route, Navigate, useLocation } from 'react-router'
import { motion } from 'framer-motion'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Toaster } from 'sonner'

import { QueryClientProvider } from '@tanstack/react-query'

import { useAppStore } from './store/useAppStore'
import { queryClient, addTranscript } from './lib/queries'
import type { Transcript } from './types'
import { EVENTS } from './lib/events'
import { ROUTES } from './lib/routes'
import { useEventListener, useDelayedFlag } from './lib/hooks'
import { Layout } from './components/Layout'
import { ErrorBoundary } from './components/ErrorBoundary'
import { Dashboard } from './pages/Dashboard'
import { SettingsPage as Settings, DictionaryPage as Dictionary, preloadRoute } from './lib/routeModules'
import { showMainWindow } from './lib/showMainWindow'

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

// Shown once, on first run. Eager, it put its whole tree on every startup.
const ModelPickerModal = lazy(() => import('./components/ModelPickerModal').then(m => ({ default: m.ModelPickerModal })))

function App() {
  const { theme, starting, modelChosen, startup, listenForModelEvents } = useAppStore(useShallow(s => ({
    theme: s.theme,
    starting: s.starting,
    modelChosen: s.modelChosen,
    startup: s.startup,
    listenForModelEvents: s.listenForModelEvents,
  })))
  const [initialRoute] = useState(() => useAppStore.getState().activeRoute)

  useEffect(() => {
    if (starting) return
    // Let the themed, populated page paint before revealing the native window.
    let frame = requestAnimationFrame(() => { frame = requestAnimationFrame(showMainWindow) })
    return () => cancelAnimationFrame(frame)
  }, [starting])

  useEffect(() => {
    if (starting) return
    const preload = () => { preloadRoute(ROUTES.SETTINGS); preloadRoute(ROUTES.DICTIONARY) }
    if ('requestIdleCallback' in window) {
      const id = window.requestIdleCallback(preload, { timeout: 1000 })
      return () => window.cancelIdleCallback(id)
    }
    const id = setTimeout(preload, 200)
    return () => clearTimeout(id)
  }, [starting])

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

  useEventListener<Transcript>(EVENTS.TRANSCRIPT_NEW, addTranscript)

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
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <AppRoutes initialRoute={initialRoute} />
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
    </QueryClientProvider>
  )
}

function AppRoutes({ initialRoute }: { initialRoute: string }) {
  const location = useLocation()
  return (
    <Routes location={location}>
      <Route path={ROUTES.DASHBOARD} element={<Layout />}>
        <Route index element={
          <ErrorBoundary>
            <Dashboard />
          </ErrorBoundary>
        } />
        <Route path={ROUTES.SETTINGS.slice(1)} element={
          <ErrorBoundary>
            <Suspense fallback={<RouteFallback />}><Settings /></Suspense>
          </ErrorBoundary>
        } />
        <Route path={ROUTES.DICTIONARY.slice(1)} element={
          <ErrorBoundary>
            <Suspense fallback={<RouteFallback />}><Dictionary /></Suspense>
          </ErrorBoundary>
        } />
        <Route path="*" element={<Navigate to={initialRoute} replace />} />
      </Route>
    </Routes>
  )
}

export default App
