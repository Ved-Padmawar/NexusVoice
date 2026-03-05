import { useState } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Alert, AlertDescription } from '@/components/ui/alert'

type Mode = 'login' | 'register'

const loginSchema = z.object({
  email: z.string().email('Enter a valid email address'),
  password: z.string().min(1, 'Password is required'),
  rememberMe: z.boolean(),
})

const registerSchema = z.object({
  email: z.string().email('Enter a valid email address'),
  password: z
    .string()
    .min(8, 'Password must be at least 8 characters')
    .regex(/[A-Z]/, 'Must contain an uppercase letter')
    .regex(/[0-9]/, 'Must contain a number'),
  rememberMe: z.boolean(),
})

type FormValues = z.infer<typeof loginSchema>

export function Auth() {
  const [mode, setMode] = useState<Mode>('login')
  const { login, register, error, setError } = useAppStore()
  const navigate = useNavigate()
  const location = useLocation()

  const from = (location.state as { from?: { pathname: string } })?.from?.pathname ?? '/'

  const {
    register: formRegister,
    handleSubmit,
    formState: { errors, isSubmitting },
    reset,
    setError: setFieldError,
  } = useForm<FormValues>({
    resolver: zodResolver(mode === 'login' ? loginSchema : registerSchema),
    defaultValues: { email: '', password: '', rememberMe: false },
  })

  const onSubmit = async (data: FormValues) => {
    setError(null)
    try {
      if (mode === 'login') {
        await login(data.email, data.password)
      } else {
        await register(data.email, data.password)
      }
      navigate(from, { replace: true })
    } catch {
      setFieldError('root', { message: 'Authentication failed. Please try again.' })
    }
  }

  const switchMode = (next: Mode) => {
    setMode(next)
    setError(null)
    reset({ email: '', password: '', rememberMe: false })
  }

  const storeError = error

  return (
    <div className="auth-page">
      {/* Left — brand panel */}
      <div className="auth-left">
        <div className="auth-left-brand">
          <div className="auth-logo">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
              <path d="M13 2L4.5 13.5H11L10 22L19.5 10.5H13L13 2Z" />
            </svg>
          </div>
          <span className="auth-brand-name">NexusVoice</span>
        </div>

        <div className="auth-left-body">
          <h2 className="auth-left-headline">Voice-to-text,<br />reimagined.</h2>
          <p className="auth-left-sub">
            Press a hotkey, speak naturally, and watch your words appear instantly — anywhere on your screen.
          </p>
        </div>

        <span className="auth-left-footer">© 2026 NexusVoice</span>
      </div>

      {/* Right — form panel */}
      <div className="auth-right">
        <div className="auth-card">
          {/* Heading */}
          <div className="auth-heading">
            <h1 className="auth-title">
              {mode === 'login' ? 'Welcome back' : 'Create account'}
            </h1>
            <p className="auth-subtitle">
              {mode === 'login'
                ? 'Sign in to continue to NexusVoice'
                : 'Get started with NexusVoice for free'}
            </p>
          </div>

          {/* Errors */}
          {storeError && (
            <Alert variant="destructive">
              <AlertDescription className="flex items-center justify-between gap-2">
                <span>{storeError}</span>
                <button
                  type="button"
                  onClick={() => setError(null)}
                  aria-label="Dismiss"
                  className="shrink-0 opacity-60 hover:opacity-100 text-lg leading-none"
                >
                  ×
                </button>
              </AlertDescription>
            </Alert>
          )}
          {errors.root && (
            <Alert variant="destructive">
              <AlertDescription>{errors.root.message}</AlertDescription>
            </Alert>
          )}

          <form onSubmit={handleSubmit(onSubmit)} className="auth-form" noValidate>
            <div className="auth-field">
              <Label htmlFor="auth-email">Email address</Label>
              <Input
                id="auth-email"
                type="email"
                placeholder="you@example.com"
                autoComplete="email"
                autoFocus
                disabled={isSubmitting}
                aria-invalid={!!errors.email}
                {...formRegister('email')}
              />
              {errors.email && (
                <p className="auth-field-error" role="alert">{errors.email.message}</p>
              )}
            </div>

            <div className="auth-field">
              <Label htmlFor="auth-password">Password</Label>
              <Input
                id="auth-password"
                type="password"
                placeholder="••••••••"
                autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
                disabled={isSubmitting}
                aria-invalid={!!errors.password}
                {...formRegister('password')}
              />
              {errors.password && (
                <p className="auth-field-error" role="alert">{errors.password.message}</p>
              )}
              {mode === 'register' && !errors.password && (
                <p className="auth-field-hint">Min. 8 characters, one uppercase, one number.</p>
              )}
            </div>

            <div className="auth-remember">
              <input
                id="auth-remember"
                type="checkbox"
                className="auth-checkbox"
                disabled={isSubmitting}
                {...formRegister('rememberMe')}
              />
              <Label htmlFor="auth-remember" className="auth-remember-label">
                Remember me for 30 days
              </Label>
            </div>

            <Button type="submit" className="w-full" disabled={isSubmitting}>
              {isSubmitting
                ? (mode === 'login' ? 'Signing in…' : 'Creating account…')
                : (mode === 'login' ? 'Sign in' : 'Create account')}
            </Button>
          </form>

          <p className="auth-switch">
            {mode === 'login' ? "Don't have an account?" : 'Already have an account?'}{' '}
            <button
              type="button"
              className="auth-switch-link"
              onClick={() => switchMode(mode === 'login' ? 'register' : 'login')}
            >
              {mode === 'login' ? 'Create one' : 'Sign in'}
            </button>
          </p>
        </div>
      </div>
    </div>
  )
}
