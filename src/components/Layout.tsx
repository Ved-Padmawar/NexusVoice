import { Outlet, Link, useLocation, useNavigate } from 'react-router'
import { useEffect, useCallback, type ReactNode } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import clsx from 'clsx'
import { LayoutDashboard, BookOpen, Settings2, LogOut, Zap, X, AlertCircle, ArrowUp, Download, RotateCcw } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from '../store/useAppStore'
import { ROUTES } from '../lib/routes'
import logoUrl from '../assets/logo.png'

function TitleBar() {
  const win = getCurrentWindow()
  return (
    <div className="absolute top-0 left-0 right-0 flex items-stretch h-8 z-20 select-none pointer-events-none">
      <div className="flex-1 h-full cursor-default pointer-events-auto" data-tauri-drag-region />
      <div className="flex items-stretch no-drag pointer-events-auto">
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
  const { downloadError } = useAppStore()

  return (
    <>
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

/** Update prompt above the account row. Installs in place — the About tab is
 *  the secondary path, not the only one. */
function UpdatePrompt() {
  const status = useAppStore(s => s.updateStatus)
  const version = useAppStore(s => s.updateVersion)
  const progress = useAppStore(s => s.updateProgress)
  const dismissed = useAppStore(s => s.updateDismissed)
  const installUpdate = useAppStore(s => s.installUpdate)
  const restartForUpdate = useAppStore(s => s.restartForUpdate)
  const dismissUpdate = useAppStore(s => s.dismissUpdate)

  const shown = !dismissed && (status === 'available' || status === 'downloading' || status === 'ready')

  return (
    <AnimatePresence initial={false}>
      {shown && (
        <motion.div
          key="update-prompt"
          initial={{ opacity: 0, height: 0 }}
          animate={{ opacity: 1, height: 'auto' }}
          exit={{ opacity: 0, height: 0 }}
          transition={{ duration: 0.2 }}
          className="overflow-hidden px-2 pb-2"
        >
          <div
            className="relative flex flex-col gap-2.25 px-2.75 py-2.5 rounded-(--r-md) bg-(--surface) border overflow-hidden"
            style={{
              borderColor: 'color-mix(in srgb, var(--accent) 22%, var(--border-soft))',
              boxShadow: '0 1px 2px oklch(0 0 0 / 0.18)',
            }}
          >
            <div className="flex items-center gap-2 pr-4.5">
              <span
                className="grid place-items-center w-5.5 h-5.5 shrink-0 rounded-(--r-sm) text-(--accent)"
                style={{ background: 'color-mix(in srgb, var(--accent) 15%, transparent)' }}
              >
                <ArrowUp size={11} strokeWidth={2.25} />
              </span>
              <span className="flex flex-col gap-px flex-1 min-w-0">
                <span className="text-[11px] font-semibold tracking-[-0.01em] text-(--fg) truncate">
                  {status === 'ready' ? 'Update installed' : status === 'downloading' ? 'Downloading' : 'Update available'}
                </span>
                <span className="flex items-center gap-1 text-[10px] font-medium text-muted-foreground tabular-nums whitespace-nowrap overflow-hidden">
                  {status === 'available' ? (
                    <>
                      <span>v{__APP_VERSION__}</span>
                      <span className="opacity-50">→</span>
                      <span className="text-(--accent) font-semibold">v{version}</span>
                    </>
                  ) : status === 'ready' ? (
                    <>
                      <span className="text-(--accent) font-semibold">v{version}</span>
                      <span>· restart to finish</span>
                    </>
                  ) : (
                    <span className="text-(--accent) font-semibold">v{version}</span>
                  )}
                </span>
              </span>
            </div>

            {status !== 'downloading' && (
              <button
                type="button"
                title="Dismiss"
                onClick={dismissUpdate}
                className="absolute top-1.25 right-1.25 flex items-center justify-center w-4 h-4 rounded-(--r-sm) text-muted-foreground bg-transparent border-none cursor-pointer transition-colors duration-(--t-fast) hover:text-destructive hover:bg-(--danger-soft)"
              >
                <X size={10} strokeWidth={2} />
              </button>
            )}

            {status === 'downloading' ? (
              <div className="flex flex-col gap-1.25">
                <div
                  className="h-0.75 rounded-full overflow-hidden"
                  style={{ background: 'color-mix(in srgb, var(--fg) 12%, transparent)' }}
                >
                  <motion.div
                    className="h-full rounded-full bg-(--accent)"
                    animate={{ width: `${progress}%` }}
                    transition={{ duration: 0.24, ease: [0.4, 0, 0.2, 1] }}
                  />
                </div>
                <div className="flex items-baseline justify-between gap-2">
                  <span className="text-[10px] text-muted-foreground tabular-nums">Downloading…</span>
                  <span className="text-[10px] font-semibold text-(--fg-2) tabular-nums">{progress}%</span>
                </div>
              </div>
            ) : (
              <motion.button
                type="button"
                onClick={status === 'ready' ? restartForUpdate : installUpdate}
                className={`w-full h-6.25 inline-flex items-center justify-center gap-1.25 rounded-(--r-sm) text-[11px] font-semibold cursor-pointer ${
                  status === 'ready'
                    ? 'bg-transparent text-(--accent) border'
                    : 'bg-(--accent) text-primary-foreground border-none'
                }`}
                style={status === 'ready'
                  ? { borderColor: 'color-mix(in srgb, var(--accent) 40%, transparent)' }
                  : undefined}
                whileHover={status === 'ready'
                  ? { backgroundColor: 'color-mix(in srgb, var(--accent) 12%, transparent)' }
                  : { backgroundColor: 'color-mix(in srgb, var(--accent) 85%, white)' }}
                whileTap={{ scale: 0.98 }}
                transition={{ duration: 0.15 }}
              >
                {status === 'ready'
                  ? <><RotateCcw size={10} strokeWidth={2.25} />Restart now</>
                  : <><Download size={10} strokeWidth={2.25} />Install update</>}
              </motion.button>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
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
    <div className="relative flex flex-col h-dvh overflow-hidden bg-background">
      <TitleBar />
      <div className="flex flex-1 min-h-0 overflow-hidden">
        {/* Sidebar */}
        <aside className="w-(--sidebar-w) shrink-0 h-full bg-(--panel) border-r border-(--border) flex flex-col relative z-10">
          {/* Brand */}
          <div className="px-3.5 pt-9 pb-3 border-b border-(--border-soft)">
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

          <UpdatePrompt />

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
          <div className="w-full pt-8 shrink-0">
            <ModelBanner />
          </div>
          <main className="flex-1 w-full overflow-hidden flex flex-col min-h-0">
            <Outlet />
          </main>
        </div>
      </div>
    </div>
  )
}
