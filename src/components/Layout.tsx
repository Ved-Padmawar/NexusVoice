import { Outlet, Link, useLocation, useNavigate } from 'react-router-dom'
import { useEffect } from 'react'
import clsx from 'clsx'
import { LayoutDashboard, BookOpen, Settings2, LogOut, Zap, X, AlertCircle, ArrowUpCircle } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from '../store/useAppStore'

function TitleBar() {
  const win = getCurrentWindow()
  return (
    <div className="titlebar">
      <div className="titlebar__drag" data-tauri-drag-region />
      <div className="titlebar__controls">
        <button
          type="button"
          className="titlebar__btn"
          onClick={() => win.minimize()}
          aria-label="Minimize"
        >
          <svg width="10" height="1" viewBox="0 0 10 1"><rect width="10" height="1" fill="currentColor" /></svg>
        </button>
        <button
          type="button"
          className="titlebar__btn"
          onClick={() => win.toggleMaximize()}
          aria-label="Maximize"
        >
          <svg width="9" height="9" viewBox="0 0 9 9" fill="none"><rect x="0.5" y="0.5" width="8" height="8" rx="0.5" stroke="currentColor" /></svg>
        </button>
        <button
          type="button"
          className="titlebar__btn titlebar__btn--close"
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

// Uses store state — no separate event subscriptions, no re-subscription on render
function ModelBanner() {
  const { modelDownloading, downloadProgress, downloadError, modelReady } = useAppStore()

  if (modelDownloading) {
    return (
      <div className="model-banner model-banner--downloading">
        <div className="model-banner__body">
          <span className="model-banner__text">Downloading Whisper model… {downloadProgress}%</span>
          <div className="model-banner__bar-track">
            <div className="model-banner__bar-fill" style={{ width: `${downloadProgress}%` }} />
          </div>
        </div>
      </div>
    )
  }

  if (downloadError) {
    return (
      <div className="model-banner model-banner--error">
        <div className="model-banner__body">
          <AlertCircle size={13} strokeWidth={2} style={{ flexShrink: 0 }} />
          <span className="model-banner__text">Download failed: {downloadError}</span>
        </div>
        <button type="button" className="model-banner__close" onClick={() => useAppStore.setState({ downloadError: null })}>
          <X size={12} strokeWidth={2} />
        </button>
      </div>
    )
  }

  if (modelReady && downloadProgress === 100) {
    return (
      <div className="model-banner model-banner--done">
        <div className="model-banner__body">
          <span className="model-banner__text">Model ready — NexusVoice is fully operational.</span>
        </div>
        <button type="button" className="model-banner__close" onClick={() => useAppStore.setState({ downloadProgress: 0 })}>
          <X size={12} strokeWidth={2} />
        </button>
      </div>
    )
  }

  return null
}

function UpdateBanner() {
  const { updateAvailable } = useAppStore()
  const navigate = useNavigate()

  if (!updateAvailable) return null

  return (
    <div className="model-banner model-banner--update">
      <div className="model-banner__body">
        <ArrowUpCircle size={13} strokeWidth={2} style={{ flexShrink: 0 }} />
        <span className="model-banner__text">Update available — v{updateAvailable}</span>
        <button
          type="button"
          className="model-banner__action"
          onClick={() => navigate('/settings', { state: { tab: 'about' } })}
        >
          Install
        </button>
      </div>
      <button type="button" className="model-banner__close" onClick={() => useAppStore.setState({ updateAvailable: null })}>
        <X size={12} strokeWidth={2} />
      </button>
    </div>
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

  // Persist active route to store so it survives tray minimize / alt-tab
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
    <div className="app-shell">
      <TitleBar />
      <ModelBanner />
      <UpdateBanner />
      <div className="app-body">
        <aside className="sidebar">
          <div className="sidebar__header">
            <Link to="/" className="sidebar__brand">
              <div className="sidebar__logo">
                <Zap size={13} strokeWidth={2.5} />
              </div>
              <div>
                <div className="sidebar__name">NexusVoice</div>
                <div className="sidebar__version">v{__APP_VERSION__}</div>
              </div>
            </Link>
          </div>

          <nav className="sidebar__nav">
            {NAV.map(({ path, label, Icon }) => (
              <Link
                key={path}
                to={path}
                className={clsx('sidebar__link', location.pathname === path && 'sidebar__link--active')}
              >
                <Icon size={15} strokeWidth={1.75} className="sidebar__link-icon" />
                <span>{label}</span>
              </Link>
            ))}
          </nav>

          <div className="sidebar__footer">
            {user ? (
              <div className="sidebar__user">
                <div className="sidebar__avatar">{initials}</div>
                <div className="sidebar__user-info">
                  <div className="sidebar__user-email">{user.email}</div>
                  <div className="sidebar__user-role">Free plan</div>
                </div>
                <button
                  type="button"
                  onClick={handleLogout}
                  title="Log out"
                  className="sidebar__logout-btn"
                >
                  <LogOut size={13} strokeWidth={1.75} />
                </button>
              </div>
            ) : (
              <Link
                to="/auth"
                className={clsx('sidebar__link', location.pathname === '/auth' && 'sidebar__link--active')}
              >
                <Zap size={15} strokeWidth={1.75} className="sidebar__link-icon" />
                <span>Log in</span>
              </Link>
            )}
          </div>
        </aside>

        <div className="main-area">
          <main className="main-content">
            <Outlet />
          </main>
        </div>
      </div>
    </div>
  )
}
