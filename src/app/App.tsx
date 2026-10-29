import { useShallow } from 'zustand/react/shallow'
import { useEffect, useState, lazy, Suspense } from 'react'
import { BrowserRouter, Routes, Route, Navigate, useLocation } from 'react-router'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Toaster } from 'sonner'

import { useAppStore } from './store/useAppStore'
import { addTranscript } from '../features/history/queries'
import { listenForModelEvents } from '../features/models/queries'
import { listenForCudaEvents, useCuda } from '../features/models/cuda'
import { followPillSettings } from '../features/settings/queries'
import { ROUTES } from './routes'
import { useEventListener, useDelayedFlag } from '../shared/hooks'
import { Layout } from './Layout'
import { ErrorBoundary } from './ErrorBoundary'
import { Dashboard } from '../features/history/Dashboard'
import { SettingsPage as Settings, DictionaryPage as Dictionary, preloadRoute } from './routeModules'
import { showMainWindow } from './showMainWindow'
import { setWindowVisible } from '../shared/frameClock'

const Spinner = () => (
  <div className="size-7 animate-spin rounded-full border-2 border-line border-t-accent" />
)

/** Full-screen startup loader. Spinner appears only after a short delay so a fast
 * database open doesn't flash it; the themed background shows immediately. */
function StartupLoader() {
  const showSpinner = useDelayedFlag(true, 250)
  return (
    <div className="flex min-h-dvh flex-col items-center justify-center gap-3.5 bg-bg" role="status" aria-live="polite" data-tauri-drag-region>
      {showSpinner && (
        <>
          <Spinner />
          <p className="text-sm text-muted">Loading…</p>
        </>
      )}
    </div>
  )
}

/** Suspense fallback for lazy routes — delayed so quick chunk loads don't flash. */
function RouteFallback() {
  const showSpinner = useDelayedFlag(true, 250)
  return (
    <div className="grid place-items-center" role="status">
      {showSpinner && <Spinner />}
    </div>
  )
}

// Shown once, on first run. Eager, it put its whole tree on every startup.
// Opens only after first-run setup, so it loads on demand like the picker.
const CudaOfferModal = lazy(() => import('../features/models/CudaOfferModal').then(m => ({ default: m.CudaOfferModal })))
const ModelPickerModal = lazy(() => import('../features/models/ModelPickerModal').then(m => ({ default: m.ModelPickerModal })))

function App() {
  const { theme, starting, modelChosen, cudaOfferOpen, startup } = useAppStore(useShallow(s => ({
    theme: s.theme,
    starting: s.starting,
    modelChosen: s.modelChosen,
    cudaOfferOpen: s.cudaOfferOpen,
    startup: s.startup,
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
    const cleanups = [listenForModelEvents(), listenForCudaEvents()]
    return () => { cleanups.forEach(cleanup => cleanup.then(fn => fn()).catch(() => {})) }
  }, [])

  // Not tied to setup: new installs see it once the picker closes, upgrades on launch.
  const cudaKnown = useCuda().data !== undefined
  useEffect(() => {
    if (!starting && modelChosen && cudaKnown) useAppStore.getState().offerCuda()
  }, [starting, modelChosen, cudaKnown])

  useEffect(() => {
    const t = setTimeout(() => {
      useAppStore.getState().checkForUpdate().catch(() => {})
    }, 3000)
    return () => clearTimeout(t)
  }, [])

  useEventListener('transcriptSaved', addTranscript)

  useEffect(followPillSettings, [])

  useEffect(() => {
    document.documentElement.dataset.theme = theme
  }, [theme])

  useEffect(() => {
    const unlisten = getCurrentWindow().onCloseRequested(async (event) => {
      event.preventDefault()
      setWindowVisible(false)
      await getCurrentWindow().hide()
    })
    return () => { unlisten.then(fn => fn()) }
  }, [])

  // Every path that shows the window from the tray or a shortcut also focuses it.
  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) setWindowVisible(true)
    })
    return () => { unlisten.then(fn => fn()) }
  }, [])

  if (starting) {
    return <StartupLoader />
  }

  return (
    <BrowserRouter>
      <AppRoutes initialRoute={initialRoute} />
      {/* No fallback: nothing should flash on screen while the chunk loads. */}
      {cudaOfferOpen && (
        <Suspense fallback={null}>
          <CudaOfferModal />
        </Suspense>
      )}
      {!modelChosen && (
        <Suspense fallback={null}>
          <ModelPickerModal />
        </Suspense>
      )}
      <Toaster position="bottom-right" />
    </BrowserRouter>
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
