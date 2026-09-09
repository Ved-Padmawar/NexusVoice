import { Outlet, Link, useLocation, useNavigate } from 'react-router'
import { useEffect, useState } from 'react'
import clsx from 'clsx'
import { motion, AnimatePresence, useReducedMotion } from 'framer-motion'
import { LayoutDashboard, BookOpen, Settings2, X, ArrowUp, Download, RotateCcw, ChevronRight } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { getName } from '@tauri-apps/api/app'
import { useAppStore } from '../store/useAppStore'
import { ROUTES, SETTINGS_TABS } from '../lib/routes'
import { preloadRoute } from '../lib/routeModules'
import logoUrl from '../assets/logo.png'

/** In the flow at the end of the brand strip, not floating over it, so
 *  nothing downstream has to reserve a hole for it. */
function WindowControls() {
  const win = getCurrentWindow()
  const base =
    'no-drag flex h-(--bar-h) w-11 items-center justify-center text-(--muted) transition-colors duration-(--t-fast)'
  return (
    <div className="flex items-stretch self-stretch">
      <button
        type="button"
        aria-label="Minimize"
        onClick={() => win.minimize()}
        className={clsx(base, 'cursor-pointer hover:bg-(--surface-hover) hover:text-(--fg)')}
      >
        <svg width="10" height="1" viewBox="0 0 10 1" aria-hidden>
          <rect width="10" height="1" fill="currentColor" />
        </svg>
      </button>
      <button
        type="button"
        aria-label="Maximize"
        onClick={() => win.toggleMaximize()}
        className={clsx(base, 'cursor-pointer hover:bg-(--surface-hover) hover:text-(--fg)')}
      >
        <svg width="9" height="9" viewBox="0 0 9 9" fill="none" aria-hidden>
          <rect x="0.5" y="0.5" width="8" height="8" rx="1" stroke="currentColor" />
        </svg>
      </button>
      <button
        type="button"
        aria-label="Close"
        onClick={() => win.close()}
        className={clsx(base, 'cursor-pointer hover:bg-(--close-red) hover:text-white')}
      >
        <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden>
          <line x1="1" y1="1" x2="9" y2="9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          <line x1="9" y1="1" x2="1" y2="9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
      </button>
    </div>
  )
}

/** The waveform mark in its cap. */
function StatusMark() {
  return (
    <span className="status__mark" aria-hidden>
      <span /><span /><span />
    </span>
  )
}

/**
 * Whether the app is armed, said once at the top of the window rather than only
 * on the Dashboard.
 *
 * A running download outranks everything: while one is going the app is not
 * ready *yet*, which is a different thing from being unconfigured, and it is
 * the one state worth watching. Shown on every page rather than only away from
 * the Voice tab — an indicator with two visibility rules is one you stop
 * trusting, and the Voice tab's own bar is the detail view, not a duplicate.
 *
 * Every actionable state is the button that takes you to the fix.
 */
function Status() {
  const modelReady = useAppStore(s => s.modelReady)
  const hasHotkey = useAppStore(s => s.hasHotkey)
  const modelName = useAppStore(s => s.activeModelName)
  const downloads = useAppStore(s => s.downloads)
  const navigate = useNavigate()

  const fetching = Object.values(downloads).find(
    d => d.status === 'running' || d.status === 'queued',
  )

  const active = fetching
    ? { tone: 'busy', label: `Downloading ${fetching.progress}%`, tab: SETTINGS_TABS.VOICE }
    : !modelReady
      ? { tone: 'warn', label: 'No model', tab: SETTINGS_TABS.VOICE }
      : !hasHotkey
        ? { tone: 'warn', label: 'No hotkey', tab: SETTINGS_TABS.GENERAL }
        : null

  if (active) {
    return (
      <button
        type="button"
        onClick={() => navigate(ROUTES.SETTINGS, { state: { tab: active.tab } })}
        data-tone={active.tone}
        title={active.tone === 'busy' ? 'Open Settings to watch the download' : 'Open Settings to fix this'}
        className="status no-drag"
      >
        <StatusMark />
        <span className="tabular-nums">{active.label}</span>
        <ChevronRight size={11} strokeWidth={2.25} className="-mr-1 opacity-70" />
      </button>
    )
  }

  return (
    <span className="status" title={modelName ? `${modelName} loaded` : undefined}>
      <StatusMark />
      Ready
    </span>
  )
}

/** Installs in place; the About tab is the secondary path. Floats opposite
 *  the dock, so it interrupts nothing in the page. */
function UpdateCard() {
  const status = useAppStore(s => s.updateStatus)
  const version = useAppStore(s => s.updateVersion)
  const progress = useAppStore(s => s.updateProgress)
  const dismissed = useAppStore(s => s.updateDismissed)
  const installUpdate = useAppStore(s => s.installUpdate)
  const restartForUpdate = useAppStore(s => s.restartForUpdate)
  const dismissUpdate = useAppStore(s => s.dismissUpdate)

  const shown = !dismissed && (status === 'available' || status === 'downloading' || status === 'ready')
  if (!shown) return null

  const downloading = status === 'downloading'
  const ready = status === 'ready'

  return (
    <div className="absolute bottom-(--dock-gap) right-(--gutter) z-40 w-58">
      <div className="panel relative flex flex-col gap-2.5 p-3 shadow-(--shadow-lg)">
        {!downloading && (
          <button
            type="button"
            title="Dismiss"
            aria-label="Dismiss update"
            onClick={dismissUpdate}
            className="iconbtn iconbtn-danger absolute right-1.5 top-1.5 size-5 cursor-pointer"
          >
            <X size={10} strokeWidth={2.25} />
          </button>
        )}

        <div className="flex items-center gap-2 pr-5">
          <span className="grid size-6 shrink-0 place-items-center rounded-(--r-sm) bg-(--accent-soft) text-(--on-soft)">
            <ArrowUp size={12} strokeWidth={2.25} />
          </span>
          <div className="flex min-w-0 flex-col">
            <span className="truncate text-[11.5px] font-semibold text-(--fg)">
              {ready ? 'Update installed' : downloading ? 'Downloading' : 'Update available'}
            </span>
            <span className="truncate text-[10.5px] tabular-nums text-(--muted)">
              {ready ? 'Restart to finish' : `Version ${version}`}
            </span>
          </div>
        </div>

        {downloading ? (
          <div className="flex flex-col gap-1.5">
            <div className="h-1 overflow-hidden rounded-full bg-(--bg-alt)">
              <div
                className="h-full rounded-full bg-(--accent) transition-[width] duration-300 ease-out"
                style={{ width: `${progress}%` }}
              />
            </div>
            <span className="text-right text-[10.5px] font-semibold tabular-nums text-(--fg-2)">
              {progress}%
            </span>
          </div>
        ) : (
          <button
            type="button"
            onClick={ready ? restartForUpdate : installUpdate}
            className={clsx('btn btn-sm w-full', ready ? 'btn-quiet' : 'btn-primary')}
          >
            {ready
              ? <><RotateCcw size={11} strokeWidth={2.25} />Restart now</>
              : <><Download size={11} strokeWidth={2.25} />Install update</>}
          </button>
        )}
      </div>
    </div>
  )
}

const NAV = [
  { path: ROUTES.DASHBOARD,  label: 'Dashboard',  Icon: LayoutDashboard },
  { path: ROUTES.DICTIONARY, label: 'Dictionary', Icon: BookOpen },
  { path: ROUTES.SETTINGS,   label: 'Settings',   Icon: Settings2 },
]

/**
 * The floating navigation capsule. Three destinations never justified a 198px
 * column, and the column is what made the window read as a generic admin shell.
 *
 * Only the current item is labelled, so widths change on navigation — that is
 * the point: the capsule morphs the way the recording pill does. The fill is one
 * shared-layout box, so a single element travels rather than three repainting.
 */
function Dock() {
  const location = useLocation()
  const reduced = useReducedMotion()
  const spring = reduced
    ? { duration: 0 }
    : { type: 'spring' as const, stiffness: 460, damping: 40, mass: 0.8 }

  return (
    <nav
      aria-label="Primary"
      className="absolute bottom-(--dock-gap) left-1/2 z-40 -translate-x-1/2"
    >
      <motion.div layout transition={spring} className="dock no-drag">
        {NAV.map(({ path, label, Icon }) => {
          const active = location.pathname === path
          return (
            <motion.div layout transition={spring} key={path}>
              <Link
                to={path}
                aria-current={active ? 'page' : undefined}
                aria-label={label}
                title={label}
                onPointerEnter={() => preloadRoute(path)}
                onFocus={() => preloadRoute(path)}
                className="dock-item"
              >
                {active && (
                  <motion.span layoutId="dock-fill" transition={spring} className="dock-fill" />
                )}
                <Icon
                  size={14}
                  strokeWidth={active ? 2.1 : 1.9}
                  className="relative z-[1] shrink-0"
                />
                <AnimatePresence initial={false}>
                  {active && (
                    <motion.span
                      key="label"
                      initial={{ width: 0, opacity: 0 }}
                      animate={{ width: 'auto', opacity: 1 }}
                      exit={{ width: 0, opacity: 0 }}
                      transition={spring}
                      className="relative z-[1] overflow-hidden"
                    >
                      <span className="block pl-2 pr-0.5">{label}</span>
                    </motion.span>
                  )}
                </AnimatePresence>
              </Link>
            </motion.div>
          )
        })}
      </motion.div>
    </nav>
  )
}

export function Layout() {
  const setActiveRoute = useAppStore((s) => s.setActiveRoute)
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
    <div className="relative flex h-dvh flex-col overflow-hidden bg-(--bg)">
      {/* Chrome only — identity, state, window buttons — so the strip stays
          honest about being the frame rather than part of the page. */}
      <header
        data-tauri-drag-region
        className="z-30 flex h-(--bar-h) shrink-0 items-center gap-3 pl-(--gutter)"
      >
        <Link to={ROUTES.DASHBOARD} className="no-drag flex items-center gap-2.5">
          <img src={logoUrl} alt="NexusVoice" className="size-5.5 shrink-0 rounded-(--r-xs)" />
          <span className="text-[12.5px] font-semibold tracking-[-0.015em] text-(--fg)">
            NexusVoice
          </span>
        </Link>
        {isDev && (
          <span className="rounded-(--r-xs) bg-(--accent-soft) px-1.5 py-px text-[9px] font-bold leading-none tracking-[0.04em] text-(--on-soft)">
            DEV
          </span>
        )}
        <Status />
        <div className="ml-auto" />
        <WindowControls />
      </header>

      <main className="relative flex min-h-0 flex-1 flex-col overflow-hidden">
        <Outlet />

        {/* Content dissolves into the floor rather than being sliced by the dock. */}
        <div
          aria-hidden
          className="pointer-events-none absolute inset-x-0 bottom-0 z-20 h-20 bg-gradient-to-t from-(--bg) via-(--bg)/70 to-transparent"
        />
        <Dock />
        <UpdateCard />
      </main>
    </div>
  )
}
