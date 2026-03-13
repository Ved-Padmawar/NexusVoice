import { Navigate, useLocation } from 'react-router-dom'
import { useAppStore } from '../store/useAppStore'

interface AuthGuardProps {
  children: React.ReactNode
}

export function AuthGuard({ children }: AuthGuardProps) {
  const user = useAppStore((s) => s.user)
  const authChecking = useAppStore((s) => s.authChecking)
  const location = useLocation()

  if (authChecking) {
    return (
      <div className="app-loading" role="status" aria-live="polite">
        <div className="loading-spinner" />
      </div>
    )
  }

  if (!user) {
    return <Navigate to="/auth" state={{ from: location }} replace />
  }

  return <>{children}</>
}
