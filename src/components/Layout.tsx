import { Outlet, Link, useLocation } from 'react-router-dom'
import clsx from 'clsx'
import { useAppStore } from '../store/useAppStore'

const navLinkClass = (active: boolean) =>
  clsx('nav-link', active && 'nav-link-active')

export function Layout() {
  const { user, logout } = useAppStore()
  const location = useLocation()

  return (
    <div className="app-shell">
      <header className="top-bar">
        <div className="brand-block">
          <Link to="/" className="brand-link">
            <p className="kicker">NexusVoice</p>
            <h1 className="title">Realtime transcription cockpit</h1>
          </Link>
          <p className="subtitle">
            Capture, correct, and ship transcripts with confidence.
          </p>
        </div>
        <nav className="top-actions nav-actions">
          <Link to="/" className={navLinkClass(location.pathname === '/')}>
            Dashboard
          </Link>
          <Link to="/settings" className={navLinkClass(location.pathname === '/settings')}>
            Settings
          </Link>
          {user ? (
            <span className="nav-user">
              <span className="nav-email">{user.email}</span>
              <button
                type="button"
                className="nav-logout"
                onClick={logout}
              >
                Log out
              </button>
            </span>
          ) : (
            <Link to="/auth" className={navLinkClass(location.pathname === '/auth')}>
              Log in
            </Link>
          )}
        </nav>
      </header>

      <main className="main-content">
        <Outlet />
      </main>
    </div>
  )
}
