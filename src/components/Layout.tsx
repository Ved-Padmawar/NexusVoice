import { Outlet, Link, useLocation, useNavigate } from 'react-router-dom'
import { useEffect, useRef, type ReactNode } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import clsx from 'clsx'
import { LayoutDashboard, BookOpen, Settings2, LogOut, Zap, X, AlertCircle, ArrowUpCircle } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from '../store/useAppStore'

function TitleBar() {
  const win = getCurrentWindow()
  return (
    <div className="flex items-stretch h-8 flex-shrink-0 bg-[var(--panel)] border-b border-[var(--border)] select-none">
      <div className="flex-1 h-full cursor-default" data-tauri-drag-region />
      <div className="flex items-stretch no-drag">
        <button
          type="button"
          className="flex items-center justify-center w-[46px] h-full bg-transparent border-none cursor-pointer text-[var(--muted)] transition-[background,color] duration-[var(--t-fast)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)]"
          onClick={() => win.minimize()}
          aria-label="Minimize"
        >
          <svg width="10" height="1" viewBox="0 0 10 1"><rect width="10" height="1" fill="currentColor" /></svg>
        </button>
        <button
          type="button"
          className="flex items-center justify-center w-[46px] h-full bg-transparent border-none cursor-pointer text-[var(--muted)] transition-[background,color] duration-[var(--t-fast)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)]"
          onClick={() => win.toggleMaximize()}
          aria-label="Maximize"
        >
          <svg width="9" height="9" viewBox="0 0 9 9" fill="none"><rect x="0.5" y="0.5" width="8" height="8" rx="0.5" stroke="currentColor" /></svg>
        </button>
        <button
          type="button"
          className="flex items-center justify-center w-[46px] h-full bg-transparent border-none cursor-pointer text-[var(--muted)] transition-[background,color] duration-[var(--t-fast)] hover:bg-[#c42b1c] hover:text-white"
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
  const { modelDownloading, downloadProgress, downloadError, modelReady } = useAppStore()
  const autoDismissRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const showReady = modelReady && downloadProgress === 100

  useEffect(() => {
    if (showReady) {
      autoDismissRef.current = setTimeout(() => {
        useAppStore.setState({ downloadProgress: 0 })
      }, 3000)
    }
    return () => { if (autoDismissRef.current) clearTimeout(autoDismissRef.current) }
  }, [showReady])

  return (
    <>
      <SlideBanner visible={modelDownloading}>
        <div className="flex items-center gap-[10px] px-[14px] py-[7px] flex-shrink-0 text-[12px] border-b border-[var(--accent)] bg-[var(--accent-soft)] text-[var(--fg)]">
          <div className="flex items-center gap-[10px] flex-1 min-w-0">
            <span className="whitespace-nowrap overflow-hidden text-ellipsis text-[var(--fg-2)]">Downloading Whisper model… {downloadProgress}%</span>
            <div className="flex-1 h-[3px] rounded-full bg-[var(--border)] overflow-hidden min-w-[60px]">
              <div className="h-full rounded-full bg-[var(--accent)] transition-[width] duration-300 ease-linear" style={{ width: `${downloadProgress}%` }} />
            </div>
          </div>
        </div>
      </SlideBanner>

      <SlideBanner visible={!!downloadError}>
        <div
          className="flex items-center gap-[10px] px-[14px] py-[7px] flex-shrink-0 text-[12px] border-b text-[var(--fg)]"
          style={{ background: 'color-mix(in srgb, #e05555 12%, transparent)', borderColor: 'color-mix(in srgb, #e05555 40%, transparent)' }}
        >
          <div className="flex items-center gap-[10px] flex-1 min-w-0">
            <AlertCircle size={13} strokeWidth={2} className="flex-shrink-0" />
            <span className="whitespace-nowrap overflow-hidden text-ellipsis text-[var(--fg-2)]">Download failed: {downloadError}</span>
          </div>
          <button
            type="button"
            className="flex items-center justify-center w-5 h-5 rounded-[var(--r-sm)] text-[var(--muted)] bg-transparent border-none cursor-pointer flex-shrink-0 transition-[color,background] duration-[var(--t-fast)] hover:text-[var(--fg)] hover:bg-[var(--surface-hover)]"
            onClick={() => useAppStore.setState({ downloadError: null })}
          >
            <X size={12} strokeWidth={2} />
          </button>
        </div>
      </SlideBanner>

      <SlideBanner visible={showReady}>
        <div
          className="flex items-center gap-[10px] px-[14px] py-[7px] flex-shrink-0 text-[12px] border-b text-[var(--fg)]"
          style={{ background: 'color-mix(in srgb, #3d9e6a 12%, transparent)', borderColor: 'color-mix(in srgb, #3d9e6a 40%, transparent)' }}
        >
          <div className="flex items-center gap-[10px] flex-1 min-w-0">
            <span className="whitespace-nowrap overflow-hidden text-ellipsis text-[var(--fg-2)]">Model ready — NexusVoice is fully operational.</span>
          </div>
          <button
            type="button"
            className="flex items-center justify-center w-5 h-5 rounded-[var(--r-sm)] text-[var(--muted)] bg-transparent border-none cursor-pointer flex-shrink-0 transition-[color,background] duration-[var(--t-fast)] hover:text-[var(--fg)] hover:bg-[var(--surface-hover)]"
            onClick={() => useAppStore.setState({ downloadProgress: 0 })}
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
        className="flex items-center gap-[10px] px-[14px] py-[7px] flex-shrink-0 text-[12px] border-b text-[var(--accent)]"
        style={{ background: 'color-mix(in srgb, var(--accent) 10%, transparent)', borderColor: 'color-mix(in srgb, var(--accent) 35%, transparent)' }}
      >
        <div className="flex items-center gap-[10px] flex-1 min-w-0">
          <ArrowUpCircle size={13} strokeWidth={2} className="flex-shrink-0" />
          <span className="whitespace-nowrap overflow-hidden text-ellipsis">Update available — v{updateAvailable}</span>
          <button
            type="button"
            className="flex-shrink-0 text-[11px] font-semibold text-[var(--accent)] bg-[var(--accent-soft)] border border-[var(--accent)] rounded-[var(--r-sm)] px-2 py-0 cursor-pointer leading-[18px] transition-[background,color] duration-[var(--t-fast)] hover:bg-[var(--accent)] hover:text-[var(--accent-fg)]"
            style={{ borderColor: 'color-mix(in srgb, var(--accent) 35%, transparent)' }}
            onClick={() => navigate('/settings', { state: { tab: 'about' } })}
          >
            Install
          </button>
        </div>
        <button
          type="button"
          className="flex items-center justify-center w-5 h-5 rounded-[var(--r-sm)] text-[var(--muted)] bg-transparent border-none cursor-pointer flex-shrink-0 transition-[color,background] duration-[var(--t-fast)] hover:text-[var(--fg)] hover:bg-[var(--surface-hover)]"
          onClick={() => useAppStore.setState({ updateAvailable: null })}
        >
          <X size={12} strokeWidth={2} />
        </button>
      </div>
    </SlideBanner>
  )
}

const NAV = [
  { path: '/',           label: 'Dashboard',  Icon: LayoutDashboard },
  { path: '/dictionary', label: 'Dictionary', Icon: BookOpen },
  { path: '/settings',   label: 'Settings',   Icon: Settings2 },
]

export function Layout() {
  const { user, logout, setActiveRoute } = useAppStore()
  const location = useLocation()
  const navigate = useNavigate()

  useEffect(() => {
    if (location.pathname !== '/auth') {
      setActiveRoute(location.pathname)
    }
  }, [location.pathname, setActiveRoute])

  if (location.pathname === '/auth') return <Outlet />

  const initials = user?.email?.charAt(0).toUpperCase() ?? '?'

  const handleLogout = async () => {
    await logout()
    navigate('/auth', { replace: true })
  }

  return (
    <div className="flex flex-col h-dvh overflow-hidden bg-[var(--bg)]">
      <TitleBar />
      <ModelBanner />
      <UpdateBanner />
      <div className="flex flex-1 min-h-0 overflow-hidden">
        {/* Sidebar */}
        <aside className="w-[var(--sidebar-w)] flex-shrink-0 h-full bg-[var(--panel)] border-r border-[var(--border)] flex flex-col relative z-10">
          {/* Brand */}
          <div className="px-[14px] pt-4 pb-3 border-b border-[var(--border-soft)]">
            <Link to="/" className="flex items-center gap-[9px] no-underline group">
              <div className="w-7 h-7 rounded-[var(--r-md)] bg-[var(--accent)] flex items-center justify-center text-[var(--accent-fg)] flex-shrink-0 shadow-[var(--glow)] transition-shadow duration-[var(--t-fast)] group-hover:shadow-[var(--glow),0_0_0_3px_var(--accent-soft)]">
                <Zap size={13} strokeWidth={2.5} />
              </div>
              <div>
                <div className="text-[13px] font-bold tracking-[-0.02em] text-[var(--fg)] leading-none">NexusVoice</div>
                <div className="text-[10px] text-[var(--muted)] mt-0.5 tracking-[0.03em]">v{__APP_VERSION__}</div>
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
                    'flex items-center gap-[9px] px-[10px] py-[7px] rounded-[var(--r-md)] no-underline text-[13px] font-medium transition-[color,background] duration-[var(--t-fast)] relative group',
                    active
                      ? 'text-[var(--fg)] bg-[var(--surface)] font-semibold'
                      : 'text-[var(--muted)] hover:text-[var(--fg)] hover:bg-[var(--surface)]'
                  )}
                >
                  {/* Active indicator */}
                  {active && (
                    <span className="absolute left-0 top-1/2 -translate-y-1/2 h-4 w-[3px] rounded-r-[3px] bg-[var(--accent)]" />
                  )}
                  <Icon
                    size={15}
                    strokeWidth={1.75}
                    className={clsx(
                      'w-4 h-4 flex-shrink-0 transition-opacity duration-[var(--t-fast)]',
                      active ? 'opacity-100' : 'opacity-65 group-hover:opacity-100'
                    )}
                  />
                  <span>{label}</span>
                </Link>
              )
            })}
          </nav>

          {/* Footer */}
          <div className="px-2 pb-3 pt-2 border-t border-[var(--border-soft)]">
            {user ? (
              <div className="flex items-center gap-2 px-[10px] py-[7px] rounded-[var(--r-md)] bg-[var(--surface)] border border-[var(--border-soft)]">
                <div className="w-6 h-6 rounded-full bg-[var(--accent-soft)] text-[var(--accent)] flex items-center justify-center text-[10px] font-bold flex-shrink-0 uppercase border border-[var(--accent-soft)]">
                  {initials}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-[11px] text-[var(--fg-2)] whitespace-nowrap overflow-hidden text-ellipsis font-medium">{user.email}</div>
                  <div className="text-[10px] text-[var(--muted)] mt-px">Free plan</div>
                </div>
                <button
                  type="button"
                  onClick={handleLogout}
                  title="Log out"
                  className="bg-transparent border-none cursor-pointer text-[var(--muted)] p-1 rounded-[var(--r-sm)] flex items-center justify-center flex-shrink-0 transition-colors duration-[var(--t-fast)] hover:text-[var(--danger)]"
                >
                  <LogOut size={13} strokeWidth={1.75} />
                </button>
              </div>
            ) : (
              <Link
                to="/auth"
                className={clsx(
                  'flex items-center gap-[9px] px-[10px] py-[7px] rounded-[var(--r-md)] no-underline text-[13px] font-medium transition-[color,background] duration-[var(--t-fast)]',
                  location.pathname === '/auth'
                    ? 'text-[var(--fg)] bg-[var(--surface)]'
                    : 'text-[var(--muted)] hover:text-[var(--fg)] hover:bg-[var(--surface)]'
                )}
              >
                <Zap size={15} strokeWidth={1.75} className="w-4 h-4 flex-shrink-0" />
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
