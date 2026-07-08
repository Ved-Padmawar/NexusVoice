import { Outlet, Link, useLocation, useNavigate } from 'react-router-dom'
import { useEffect, useRef, useCallback, type ReactNode } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import clsx from 'clsx'
import { LayoutDashboard, BookOpen, Settings2, LogOut, Zap, X, AlertCircle, ArrowUpCircle, XCircle } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from '../store/useAppStore'
import { ROUTES } from '../lib/routes'
import logoUrl from '../assets/logo.png'

function TitleBar() {
  const win = getCurrentWindow()
  return (
    <div className="flex items-stretch h-8 shrink-0 bg-(--panel) border-b border-(--border) select-none">
      <div className="flex-1 h-full cursor-default" data-tauri-drag-region />
      <div className="flex items-stretch no-drag">
        <button
          type="button"
          className="flex items-center justify-center w-11.5 h-full bg-transparent border-none cursor-pointer text-muted-foreground transition-[background,color] duration-(--t-fast) hover:bg-accent hover:text-(--fg)"
          onClick={() => win.minimize()}
          aria-label="Minimize"
        >
          <svg width="10" height="1" viewBox="0 0 10 1"><rect width="10" height="1" fill="currentColor" /></svg>
        </button>
        <button
          type="button"
          className="flex items-center justify-center w-11.5 h-full bg-transparent border-none cursor-pointer text-muted-foreground transition-[background,color] duration-(--t-fast) hover:bg-accent hover:text-(--fg)"
          onClick={() => win.toggleMaximize()}
          aria-label="Maximize"
        >
          <svg width="9" height="9" viewBox="0 0 9 9" fill="none"><rect x="0.5" y="0.5" width="8" height="8" rx="0.5" stroke="currentColor" /></svg>
        </button>
        <button
          type="button"
          className="flex items-center justify-center w-11.5 h-full bg-transparent border-none cursor-pointer text-muted-foreground transition-[background,color] duration-(--t-fast) hover:bg-(--color-close) hover:text-white"
          onClick={() => win.close()}
          aria-label="Close"
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
            <line x1="1" y1="1" x2="9" y2="9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            <line x1="9" y1="1" x2="1" y2="9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
        </button>
      </div>
    </div>
  )
}

function SlideBanner({ visible, children }: { visible: boolean; children: ReactNode }) {
  return (
    <AnimatePresence initial={false}>
      {visible && (
        <motion.div
          key="banner"
          initial={{ height: 0, opacity: 0 }}
          animate={{ height: 'auto', opacity: 1 }}
          exit={{ height: 0, opacity: 0 }}
          transition={{ duration: 0.25, ease: 'easeInOut' }}
          style={{ overflow: 'hidden' }}
        >
          {children}
        </motion.div>
      )}
    </AnimatePresence>
  )
}

function ModelBanner() {
  const { modelDownloading, downloadProgress, downloadError, modelReady, modelChosen, cancelDownload } = useAppStore()
  const autoDismissRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  // Suppress banner during first-run modal — modal handles its own progress display
  const bannerActive = modelChosen && modelDownloading

  // Auto-reset progress when download completes so the downloading banner hides
  useEffect(() => {
    if (modelReady && downloadProgress === 100) {
      autoDismissRef.current = setTimeout(() => {
        useAppStore.setState({ downloadProgress: 0 })
      }, 500)
    }
    return () => { if (autoDismissRef.current) clearTimeout(autoDismissRef.current) }
  }, [modelReady, downloadProgress])

  return (
    <>
      <SlideBanner visible={bannerActive}>
        <div className="flex items-center gap-2.5 px-3.5 py-1.75 shrink-0 text-[12px] border-b border-(--accent) bg-(--accent-soft) text-(--fg)">
          <div className="flex items-center gap-2.5 flex-1 min-w-0">
            <span className="whitespace-nowrap overflow-hidden text-ellipsis text-(--fg-2)">Downloading NVIDIA model… {downloadProgress}%</span>
            <div className="flex-1 h-0.75 rounded-full bg-(--border) overflow-hidden min-w-15">
              <div className="h-full rounded-full bg-(--accent) transition-[width] duration-300 ease-linear" style={{ width: `${downloadProgress}%` }} />
            </div>
          </div>
          <button
            type="button"
            className="flex items-center gap-1 shrink-0 text-[11px] font-medium text-muted-foreground hover:text-destructive transition-colors duration-(--t-fast) cursor-pointer bg-transparent border-none px-1"
            onClick={cancelDownload}
            aria-label="Cancel download"
          >
            <XCircle size={13} strokeWidth={1.75} />
            Cancel
          </button>
        </div>
      </SlideBanner>

      <SlideBanner visible={!!downloadError}>
        <div
          className="flex items-center gap-2.5 px-3.5 py-1.75 shrink-0 text-[12px] border-b text-(--fg)"
          style={{ background: 'var(--danger-soft)', borderColor: 'color-mix(in srgb, var(--danger) 40%, transparent)' }}
        >
          <div className="flex items-center gap-2.5 flex-1 min-w-0">
            <AlertCircle size={13} strokeWidth={2} className="shrink-0" />
            <span className="whitespace-nowrap overflow-hidden text-ellipsis text-(--fg-2)">Download failed: {downloadError}</span>
          </div>
          <button
            type="button"
            className="flex items-center justify-center w-5 h-5 rounded-(--r-sm) text-muted-foreground bg-transparent border-none cursor-pointer shrink-0 transition-[color,background] duration-(--t-fast) hover:text-(--fg) hover:bg-accent"
            onClick={() => useAppStore.setState({ downloadError: null })}
          >
            <X size={12} strokeWidth={2} />
          </button>
        </div>
      </SlideBanner>

    </>
  )
}

function UpdateBanner() {
  const { updateAvailable } = useAppStore()
  const navigate = useNavigate()

  return (
    <SlideBanner visible={!!updateAvailable}>
      <div
        className="flex items-center gap-2.5 px-3.5 py-1.75 shrink-0 text-[12px] border-b text-(--accent)"
        style={{ background: 'color-mix(in srgb, var(--accent) 10%, transparent)', borderColor: 'color-mix(in srgb, var(--accent) 35%, transparent)' }}
      >
        <div className="flex items-center gap-2.5 flex-1 min-w-0">
          <ArrowUpCircle size={13} strokeWidth={2} className="shrink-0" />
          <span className="whitespace-nowrap overflow-hidden text-ellipsis">Update available — v{updateAvailable}</span>
          <button
            type="button"
            className="shrink-0 text-[11px] font-semibold text-(--accent) bg-(--accent-soft) border border-(--accent) rounded-(--r-sm) px-2 py-0 cursor-pointer leading-4.5 transition-[background,color] duration-(--t-fast) hover:bg-(--accent) hover:text-primary-foreground"
            style={{ borderColor: 'color-mix(in srgb, var(--accent) 35%, transparent)' }}
            onClick={() => navigate(ROUTES.SETTINGS, { state: { tab: 'about' } })}
          >
            Install
          </button>
        </div>
        <button
          type="button"
          className="flex items-center justify-center w-5 h-5 rounded-(--r-sm) text-muted-foreground bg-transparent border-none cursor-pointer shrink-0 transition-[color,background] duration-(--t-fast) hover:text-(--fg) hover:bg-accent"
          onClick={() => useAppStore.setState({ updateAvailable: null })}
        >
          <X size={12} strokeWidth={2} />
        </button>
      </div>
    </SlideBanner>
  )
}

const NAV = [
  { path: ROUTES.DASHBOARD,  label: 'Dashboard',  Icon: LayoutDashboard },
  { path: ROUTES.DICTIONARY, label: 'Dictionary', Icon: BookOpen },
  { path: ROUTES.SETTINGS,   label: 'Settings',   Icon: Settings2 },
]

export function Layout() {
  const { user, logout, setActiveRoute } = useAppStore()
  const location = useLocation()
  const navigate = useNavigate()

  useEffect(() => {
    if (location.pathname !== ROUTES.AUTH) {
      setActiveRoute(location.pathname)
    }
  }, [location.pathname, setActiveRoute])

  const handleLogout = useCallback(async () => {
    await logout()
    navigate('/auth', { replace: true })
  }, [logout, navigate])

  if (location.pathname === ROUTES.AUTH) return <Outlet />

  const initials = user?.email?.charAt(0).toUpperCase() ?? '?'

  return (
    <div className="flex flex-col h-dvh overflow-hidden bg-background">
      <TitleBar />
      <ModelBanner />
      <UpdateBanner />
      <div className="flex flex-1 min-h-0 overflow-hidden">
        {/* Sidebar */}
        <aside className="w-(--sidebar-w) shrink-0 h-full bg-(--panel) border-r border-(--border) flex flex-col relative z-10">
          {/* Brand */}
          <div className="px-3.5 pt-4 pb-3 border-b border-(--border-soft)">
            <Link to="/" className="flex items-center gap-2.25 no-underline group">
              {/* Real app icon (cyan/steel split tile + waveform) — same mark as
                  the taskbar/tray icon, so branding is consistent everywhere. */}
              <img
                src={logoUrl}
                alt="NexusVoice"
                className="w-9 h-9 rounded-(--r-md) shrink-0"
              />
              <div>
                <div className="text-[13px] font-black tracking-[-0.02em] leading-none"><span className="text-(--fg)">Nexus</span><span className="text-(--accent)">Voice</span></div>
                <div className="text-[10px] text-(--fg-2) mt-0.5 tracking-[0.03em]">v{__APP_VERSION__}</div>
              </div>
            </Link>
          </div>

          {/* Nav */}
          <nav className="flex-1 p-2 flex flex-col gap-px overflow-y-auto">
            {NAV.map(({ path, label, Icon }) => {
              const active = location.pathname === path
              return (
                <Link
                  key={path}
                  to={path}
                  className={clsx(
                    'flex items-center gap-2.25 px-2.5 py-1.75 rounded-(--r-md) no-underline text-[13px] font-medium transition-[color,background] duration-(--t-fast) relative group',
                    active
                      ? 'text-(--fg) bg-(--surface) font-semibold'
                      : 'text-muted-foreground hover:text-(--fg) hover:bg-(--surface)'
                  )}
                >
                  {/* Active indicator */}
                  {active && (
                    <span className="absolute left-0 top-1/2 -translate-y-1/2 h-4 w-0.75 rounded-r-[3px] bg-(--accent)" />
                  )}
                  <Icon
                    size={15}
                    strokeWidth={1.75}
                    className={clsx(
                      'w-4 h-4 shrink-0 transition-opacity duration-(--t-fast)',
                      active ? 'opacity-100' : 'opacity-65 group-hover:opacity-100'
                    )}
                  />
                  <span>{label}</span>
                </Link>
              )
            })}
          </nav>

          {/* Footer */}
          <div className="px-2 pb-3 pt-2 border-t border-(--border-soft)">
            {user ? (
              <div className="flex items-center gap-2 px-2.5 py-1.75 rounded-(--r-md) bg-(--surface) border border-(--border-soft)">
                <div className="w-6 h-6 rounded-full bg-(--accent-soft) text-(--accent) flex items-center justify-center text-[10px] font-bold shrink-0 uppercase border border-(--accent-soft)">
                  {initials}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-[11px] text-(--fg-2) whitespace-nowrap overflow-hidden text-ellipsis font-medium">{user.email}</div>
                </div>
                <button
                  type="button"
                  onClick={handleLogout}
                  title="Log out"
                  className="bg-transparent border-none cursor-pointer text-muted-foreground p-1 rounded-(--r-sm) flex items-center justify-center shrink-0 transition-colors duration-(--t-fast) hover:text-destructive"
                >
                  <LogOut size={13} strokeWidth={1.75} />
                </button>
              </div>
            ) : (
              <Link
                to="/auth"
                className={clsx(
                  'flex items-center gap-2.25 px-2.5 py-1.75 rounded-(--r-md) no-underline text-[13px] font-medium transition-[color,background] duration-(--t-fast)',
                  location.pathname === ROUTES.AUTH
                    ? 'text-(--fg) bg-(--surface)'
                    : 'text-muted-foreground hover:text-(--fg) hover:bg-(--surface)'
                )}
              >
                <Zap size={15} strokeWidth={1.75} className="w-4 h-4 shrink-0" />
                <span>Log in</span>
              </Link>
            )}
          </div>
        </aside>

        {/* Main */}
        <div className="flex-1 min-w-0 h-full overflow-hidden flex flex-col items-center">
          <main className="flex-1 w-full overflow-hidden flex flex-col min-h-0">
            <Outlet />
          </main>
        </div>
      </div>
    </div>
  )
}
