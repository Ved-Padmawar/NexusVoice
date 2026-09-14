import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { Outlet, useLocation } from 'react-router'
import { useAppStore } from '../../store/useAppStore'
import { TitleBar } from './TitleBar'

/** Fixed title bar over one scroll surface; the body never scrolls. */
export function AppShell() {
  const setActiveRoute = useAppStore((s) => s.setActiveRoute)
  const { pathname } = useLocation()
  const mainRef = useRef<HTMLElement>(null)
  const [scrolled, setScrolled] = useState(false)

  useEffect(() => {
    setActiveRoute(pathname)
  }, [pathname, setActiveRoute])

  // Layout effect: no flash at the previous route's offset.
  useLayoutEffect(() => {
    if (mainRef.current) mainRef.current.scrollTop = 0
  }, [pathname])

  return (
    <div className="nv-app" data-scrolled={scrolled || undefined}>
      <TitleBar />
      <main
        ref={mainRef}
        className="nv-main"
        onScroll={(e) => setScrolled(e.currentTarget.scrollTop > 0)}
      >
        <Outlet />
      </main>
    </div>
  )
}
