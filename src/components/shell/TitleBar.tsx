import { useEffect, useState } from 'react'
import { Link, useLocation } from 'react-router'
import { motion } from 'framer-motion'
import { BookOpen, LayoutDashboard, Settings2 } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { getName } from '@tauri-apps/api/app'
import { ROUTES } from '../../lib/routes'
import { preloadRoute } from '../../lib/routeModules'
import { UpdateChip } from './UpdateChip'
import logoUrl from '../../assets/logo.png'

const NAV = [
  { path: ROUTES.DASHBOARD,  label: 'Dashboard',  Icon: LayoutDashboard },
  { path: ROUTES.DICTIONARY, label: 'Dictionary', Icon: BookOpen },
  { path: ROUTES.SETTINGS,   label: 'Settings',   Icon: Settings2 },
]

function Nav() {
  const { pathname } = useLocation()
  return (
    <nav className="nv-nav" aria-label="Main">
      {NAV.map(({ path, label, Icon }) => {
        const active = pathname === path
        return (
          <Link
            key={path}
            to={path}
            aria-current={active ? 'page' : undefined}
            title={label}
            onPointerEnter={() => preloadRoute(path)}
            onFocus={() => preloadRoute(path)}
            className="nv-nav__item"
          >
            {active && (
              <motion.span
                layoutId="nav-indicator"
                className="nv-nav__ind"
                transition={{ type: 'spring', stiffness: 520, damping: 42 }}
              />
            )}
            <Icon strokeWidth={1.9} aria-hidden />
            <span className="nv-nav__label">{label}</span>
          </Link>
        )
      })}
    </nav>
  )
}

function WindowControls() {
  const win = getCurrentWindow()
  return (
    <div className="nv-winctl">
      <button type="button" onClick={() => win.minimize()} aria-label="Minimize">
        <svg width="10" height="1" viewBox="0 0 10 1"><rect width="10" height="1" fill="currentColor" /></svg>
      </button>
      <button type="button" onClick={() => win.toggleMaximize()} aria-label="Maximize">
        <svg width="9" height="9" viewBox="0 0 9 9" fill="none"><rect x="0.5" y="0.5" width="8" height="8" rx="0.5" stroke="currentColor" /></svg>
      </button>
      <button type="button" className="is-close" onClick={() => win.close()} aria-label="Close">
        <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
          <line x1="1" y1="1" x2="9" y2="9" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
          <line x1="9" y1="1" x2="1" y2="9" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
        </svg>
      </button>
    </div>
  )
}

/** A drag region only covers the element carrying the attribute, not its children. */
export function TitleBar() {
  const [isDev, setIsDev] = useState(false)
  useEffect(() => {
    getName()
      .then(name => setIsDev(name.toLowerCase().endsWith('dev')))
      .catch(() => setIsDev(false))
  }, [])

  return (
    <header className="nv-titlebar" data-tauri-drag-region>
      <div className="nv-titlebar__start" data-tauri-drag-region>
        <Link to={ROUTES.DASHBOARD} className="nv-brand">
          <img src={logoUrl} alt="NexusVoice" className="nv-brand__logo" />
          <span className="nv-brand__name">NexusVoice</span>
        </Link>
        {isDev && <span className="nv-badge nv-badge--warning">Dev</span>}
      </div>

      <Nav />

      <div className="nv-titlebar__end" data-tauri-drag-region>
        <UpdateChip />
        <WindowControls />
      </div>
    </header>
  )
}
