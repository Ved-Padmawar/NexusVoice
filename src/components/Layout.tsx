import { Outlet, Link, useLocation, useNavigate } from 'react-router-dom'
import { useEffect, useState } from 'react'
import clsx from 'clsx'
import { LayoutDashboard, BookOpen, Settings2, LogOut, Zap, X, AlertCircle } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { listen } from '@tauri-apps/api/event'
import { useAppStore } from '../store/useAppStore'

function TitleBar() {
  const win = getCurrentWindow()
  return (
    <div className="titlebar">
      {/* drag region — does NOT contain buttons so clicks are not swallowed */}
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

type BannerState = 'downloading' | 'done' | 'error' | null

function ModelBanner() {
  const [banner, setBanner] = useState<BannerState>(null)
  const [pct, setPct] = useState(0)
  const [errMsg, setErrMsg] = useState('')

  useEffect(() => {
    const unsubs: (() => void)[] = []
    const setup = async () => {
      unsubs.push(await listen('model-download-start', () => { setBanner('downloading'); setPct(0) }))
      unsubs.push(await listen<number>('model-download-progress', e => setPct(e.payload)))
      unsubs.push(await listen('model-download-complete', () => {
        setBanner('done')
        setTimeout(() => setBanner(null), 4000)
      }))
      unsubs.push(await listen<string>('model-download-error', e => {
        setErrMsg(e.payload)
        setBanner('error')
      }))
    }
    setup()
    return () => unsubs.forEach(fn => fn())
  }, [])

  if (!banner) return null

  return (
    <div className={`model-banner model-banner--${banner}`}>
      <div className="model-banner__body">
        {banner === 'downloading' && (
          <>
            <span className="model-banner__text">
              Downloading Whisper Large v3 Turbo… {pct}%
            </span>
            <div className="model-banner__bar-track">
              <div className="model-banner__bar-fill" style={{ width: `${pct}%` }} />
            </div>
          </>
        )}
        {banner === 'done' && (
          <span className="model-banner__text">Model ready — NexusVoice is fully operational.</span>
        )}
        {banner === 'error' && (
          <>
            <AlertCircle size={13} strokeWidth={2} style={{ flexShrink: 0 }} />
            <span className="model-banner__text">Download failed: {errMsg}</span>
          </>
        )}
      </div>
      {banner !== 'downloading' && (
        <button type="button" className="model-banner__close" onClick={() => setBanner(null)}>
          <X size={12} strokeWidth={2} />
        </button>
      )}
    </div>
  )
}

const NAV = [
  { path: '/',           label: 'Dashboard',  Icon: LayoutDashboard },
  { path: '/dictionary', label: 'Dictionary', Icon: BookOpen },
  { path: '/settings',   label: 'Settings',   Icon: Settings2 },
]

export function Layout() {
  const { user, logout } = useAppStore()
  const location = useLocation()
  const navigate = useNavigate()

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
      <div className="app-body">
      <aside className="sidebar">
        {/* Brand */}
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

        {/* Navigation */}
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

        {/* User footer */}
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
