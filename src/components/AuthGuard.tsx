import { Navigate, useLocation } from 'react-router-dom'
import { useAppStore } from '../store/useAppStore'

interface AuthGuardProps {
  children: React.ReactNode
}

/**
 * Wraps protected routes. Redirects to /auth if user is not authenticated.
 * Preserves the attempted path so we can redirect back after login.
 */
export function AuthGuard({ children }: AuthGuardProps) {
  const user = useAppStore((s) => s.user)
  const location = useLocation()

  if (!user) {
    return <Navigate to="/auth" state={{ from: location }} replace />
  }

  return <>{children}</>
}
