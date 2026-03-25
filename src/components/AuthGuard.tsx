import { type ReactNode } from 'react'
import { motion } from 'framer-motion'
import { Navigate, useLocation } from 'react-router-dom'
import { useAppStore } from '../store/useAppStore'
import { ROUTES } from '../lib/routes'

interface AuthGuardProps {
  children: ReactNode
}

export function AuthGuard({ children }: AuthGuardProps) {
  const user = useAppStore((s) => s.user)
  const authChecking = useAppStore((s) => s.authChecking)
  const location = useLocation()

  if (authChecking) {
    return (
      <div className="flex items-center justify-center min-h-dvh bg-[var(--bg)]" role="status" aria-live="polite" data-tauri-drag-region>
        <motion.div className="w-5 h-5 rounded-full border-2 border-[var(--border)] border-t-[var(--accent)]" animate={{ rotate: 360 }} transition={{ duration: 0.8, ease: 'linear', repeat: Infinity }} />
      </div>
    )
  }

  if (!user) {
    return <Navigate to={ROUTES.AUTH} state={{ from: location }} replace />
  }

  return <>{children}</>
}
