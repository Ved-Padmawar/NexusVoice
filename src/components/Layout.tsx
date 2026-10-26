import { Outlet, Link, useLocation } from 'react-router'
import { useEffect, useState } from 'react'
import { BookOpenIcon, DownloadSimpleIcon, SquaresFourIcon, MinusIcon, ArrowClockwiseIcon, SlidersHorizontalIcon, SquareIcon, XIcon } from '@phosphor-icons/react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { getName } from '@tauri-apps/api/app'
import { useAppStore } from '../store/useAppStore'
import { ROUTES } from '../lib/routes'
import { preloadRoute } from '../lib/routeModules'
import { cn } from '../lib/utils'
import logoUrl from '../assets/logo.png'

const WINDOW_BUTTON = 'grid h-8 w-11.5 cursor-pointer place-items-center text-muted transition-colors'

/** Drag region and window controls, over the content column only. The fade
 *  under it grows as the page scrolls (index.css). */
function TitleBar() {
  const win = getCurrentWindow()
  return (
    <header className="topbar relative z-20 flex items-stretch justify-end bg-bg" data-tauri-drag-region>
      <button type="button" aria-label="Minimize" onClick={() => win.minimize()} className={cn(WINDOW_BUTTON, 'no-drag hover:bg-surface-hover hover:text-fg')}>
        <MinusIcon size={13} />
      </button>
      <button type="button" aria-label="Maximize" onClick={() => win.toggleMaximize()} className={cn(WINDOW_BUTTON, 'no-drag hover:bg-surface-hover hover:text-fg')}>
        <SquareIcon size={12} />
      </button>
      <button type="button" aria-label="Close" onClick={() => win.close()} className={cn(WINDOW_BUTTON, 'no-drag hover:bg-close hover:text-white')}>
        <XIcon size={13} />
      </button>
    </header>
  )
}

const PHASE = 'col-start-1 row-start-1 transition-[opacity,filter,visibility] duration-300'
const phase = (shown: boolean) => (shown ? 'visible opacity-100 blur-0' : 'invisible opacity-0 blur-[3px]')

/** The whole update flow in one card: Install, then it fills as it downloads,
 *  then Restart. The phases share one grid cell, so the card keeps its size. */
function UpdateCard() {
  const status = useAppStore(s => s.updateStatus)
  const version = useAppStore(s => s.updateVersion)
  const progress = useAppStore(s => s.updateProgress)
  const installUpdate = useAppStore(s => s.installUpdate)
  const restartForUpdate = useAppStore(s => s.restartForUpdate)

  if (status !== 'available' && status !== 'downloading' && status !== 'ready') return null

  return (
    <div className="grid px-2 pb-2">
      <button
        type="button"
        onClick={installUpdate}
        tabIndex={status === 'available' ? 0 : -1}
        className={cn(PHASE, phase(status === 'available'), 'inline-flex h-8 cursor-pointer items-center justify-center gap-1.5 rounded-lg bg-accent-soft px-3.5 text-xs font-semibold text-accent-ink ring-1 ring-accent-line ring-inset hover:bg-accent/25 hover:ring-accent')}
      >
        <DownloadSimpleIcon size={13} weight="bold" />Install v{version}
      </button>
      <span
        role="progressbar"
        aria-valuenow={progress}
        aria-label="Downloading update"
        className={cn(PHASE, phase(status === 'downloading'), 'relative inline-flex h-8 items-center justify-center overflow-hidden rounded-lg bg-accent-soft px-3.5 text-xs font-semibold text-accent-ink tabular-nums ring-1 ring-accent-line ring-inset')}
      >
        <span className="absolute inset-y-0 left-0 bg-accent/30 transition-[width] duration-300 ease-linear" style={{ width: `${progress}%` }} />
        <span className="relative">Downloading {progress}%</span>
      </span>
      <button
        type="button"
        onClick={restartForUpdate}
        tabIndex={status === 'ready' ? 0 : -1}
        className={cn(PHASE, phase(status === 'ready'), 'inline-flex h-8 cursor-pointer items-center justify-center gap-1.5 rounded-lg bg-success-soft px-3.5 text-xs font-semibold text-success ring-1 ring-success/30 ring-inset hover:bg-success/25 hover:ring-success/60')}
      >
        <ArrowClockwiseIcon size={13} weight="bold" />Restart to update
      </button>
    </div>
  )
}

const NAV = [
  { path: ROUTES.DASHBOARD,  label: 'Dashboard',  Icon: SquaresFourIcon },
  { path: ROUTES.DICTIONARY, label: 'Dictionary', Icon: BookOpenIcon },
  { path: ROUTES.SETTINGS,   label: 'Settings',   Icon: SlidersHorizontalIcon },
]

export function Layout() {
  const setActiveRoute = useAppStore((s) => s.setActiveRoute)
  const hasHotkey = useAppStore((s) => s.hasHotkey)
  const location = useLocation()

  useEffect(() => {
    setActiveRoute(location.pathname)
  }, [location.pathname, setActiveRoute])

  const [isDev, setIsDev] = useState(false)
  useEffect(() => {
    getName()
      .then(name => setIsDev(name.toLowerCase().endsWith('dev')))
      .catch(() => setIsDev(false))
  }, [])

  return (
    <div className="app-shell grid h-dvh grid-cols-[200px_minmax(0,1fr)] overflow-hidden bg-bg text-base">
      <aside className="relative z-10 flex h-full min-h-0 flex-col border-r border-line bg-panel">
        <div className="flex items-center border-b border-line-soft px-3.5 pt-7 pb-3.5" data-tauri-drag-region>
          <Link to={ROUTES.DASHBOARD} className="no-drag flex items-center gap-2.25">
            <img src={logoUrl} alt="NexusVoice" className="size-9 shrink-0 rounded-md" />
            <div>
              <div className="flex items-center gap-1.5">
                <div className="text-[13px] leading-none font-black tracking-[-0.02em]">
                  <span className="text-fg">Nexus</span><span className="text-accent">Voice</span>
                </div>
                {isDev && (
                  <span className="rounded-xs bg-accent-soft px-1.25 py-px text-[9px] leading-none font-bold tracking-[0.06em] text-accent uppercase">Dev</span>
                )}
              </div>
              <div className="mt-0.5 text-2xs tracking-[0.03em] text-fg-2">v{__APP_VERSION__}</div>
            </div>
          </Link>
        </div>

        <nav className="flex flex-1 flex-col gap-px overflow-y-auto p-2" aria-label="Main">
          {NAV.map(({ path, label, Icon }) => {
            const active = location.pathname === path
            return (
              <Link
                key={path}
                to={path}
                aria-current={active ? 'page' : undefined}
                onPointerEnter={() => preloadRoute(path)}
                onFocus={() => preloadRoute(path)}
                className={cn(
                  'group relative flex items-center gap-2.25 rounded-md px-2.5 py-1.75 text-[13px] transition-[color,background-color] duration-120',
                  // The accent tick on the left edge grows in on the active link.
                  "before:absolute before:top-1/2 before:left-0 before:h-4 before:w-0.75 before:-translate-y-1/2 before:rounded-r-[3px] before:bg-accent before:transition-[scale] before:duration-200 before:ease-out-quint before:content-['']",
                  active
                    ? 'bg-surface font-semibold text-fg before:scale-y-100'
                    : 'font-medium text-muted before:scale-y-0 hover:bg-surface hover:text-fg',
                )}
              >
                <Icon size={15} className={cn('shrink-0', active ? 'opacity-100' : 'opacity-65 group-hover:opacity-100')} />
                {label}
                {path === ROUTES.SETTINGS && !hasHotkey && (
                  <span className="ml-auto size-1.5 rounded-full bg-warning" aria-label="No hotkey set" />
                )}
              </Link>
            )
          })}
        </nav>

        <UpdateCard />
      </aside>

      <div className="relative grid min-h-0 min-w-0 grid-rows-[32px_minmax(0,1fr)]">
        <TitleBar />
        {/* Keyed by route, so each page plays its entrance once. */}
        <main key={location.pathname} className="grid min-h-0 min-w-0 animate-[page-in_260ms_var(--ease-out-quint)]">
          <Outlet />
        </main>
      </div>
    </div>
  )
}
