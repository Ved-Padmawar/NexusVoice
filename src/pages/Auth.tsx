import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { cva } from 'class-variance-authority'
import { useAppStore } from '../store/useAppStore'

const buttonStyles = cva('btn', {
  variants: {
    tone: {
      primary: 'btn-primary',
      ghost: 'btn-ghost',
    },
    size: {
      sm: 'btn-sm',
      md: 'btn-md',
    },
  },
  defaultVariants: {
    tone: 'primary',
    size: 'md',
  },
})

type Mode = 'login' | 'register'

export function Auth() {
  const [mode, setMode] = useState<Mode>('login')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const { login, register, error, setError } = useAppStore()
  const navigate = useNavigate()

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setSubmitting(true)
    try {
      if (mode === 'login') {
        await login(email, password)
      } else {
        await register(email, password)
      }
      navigate('/')
    } catch {
      // Error stored in store
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="auth-page">
      <div className="auth-card">
        <h2 className="auth-title">{mode === 'login' ? 'Log in' : 'Register'}</h2>
        <p className="auth-subtitle">
          {mode === 'login'
            ? 'Sign in to your account'
            : 'Create an account to get started'}
        </p>

        {error && (
          <div className="error-banner" role="alert">
            {error}
          </div>
        )}

        <form onSubmit={handleSubmit} className="auth-form">
          <div className="field">
            <label className="field-label" htmlFor="auth-email">
              Email
            </label>
            <input
              id="auth-email"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="you@example.com"
              required
              autoComplete="email"
              className="input"
            />
          </div>
          <div className="field">
            <label className="field-label" htmlFor="auth-password">
              Password
            </label>
            <input
              id="auth-password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="••••••••"
              required
              autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
              className="input"
            />
          </div>
          <button
            type="submit"
            className={buttonStyles({ tone: 'primary', size: 'md' })}
            disabled={submitting}
          >
            {submitting ? 'Please wait…' : mode === 'login' ? 'Log in' : 'Register'}
          </button>
        </form>

        <p className="auth-switch">
          {mode === 'login' ? "Don't have an account?" : 'Already have an account?'}{' '}
          <button
            type="button"
            className="auth-switch-link"
            onClick={() => {
              setMode(mode === 'login' ? 'register' : 'login')
              setError(null)
            }}
          >
            {mode === 'login' ? 'Register' : 'Log in'}
          </button>
        </p>
      </div>
    </div>
  )
}
