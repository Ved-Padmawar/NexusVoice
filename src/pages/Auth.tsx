import { useState } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Zap, Check, AlertCircle, X } from 'lucide-react'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

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
    .min(8, 'At least 8 characters')
    .regex(/[A-Z]/, 'One uppercase letter required')
    .regex(/[0-9]/, 'One number required'),
  rememberMe: z.boolean(),
})

type FormValues = z.infer<typeof loginSchema>

const FEATURES = [
  'Hold a hotkey, speak naturally, release to paste',
  'On-device Whisper AI — no data leaves your machine',
  'Custom dictionary for technical terms & names',
]

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
      if (mode === 'login') await login(data.email, data.password)
      else await register(data.email, data.password)
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

  return (
    <div className="auth-page">
      {/* Left panel */}
      <div className="auth-left">
        <div className="auth-brand">
          <div className="auth-logo">
            <Zap size={15} strokeWidth={2.5} />
          </div>
          <span className="auth-brand-name">NexusVoice</span>
        </div>

        <div className="auth-hero">
          <h2 className="auth-headline">
            Voice-to-text,<br /><span>reimagined.</span>
          </h2>
          <p className="auth-subheadline">
            Speak naturally. Paste instantly. Works everywhere on your desktop.
          </p>

          <div style={{ marginTop: '28px', display: 'flex', flexDirection: 'column', gap: '10px' }}>
            {FEATURES.map((f) => (
              <div key={f} style={{ display: 'flex', alignItems: 'flex-start', gap: '10px' }}>
                <div style={{
                  width: '18px', height: '18px', borderRadius: 'var(--r-sm)',
                  background: 'var(--accent-soft)', color: 'var(--accent)',
                  display: 'flex', alignItems: 'center', justifyContent: 'center',
                  flexShrink: 0, marginTop: '1px',
                }}>
                  <Check size={10} strokeWidth={3} />
                </div>
                <span style={{ fontSize: '12px', color: 'var(--fg-2)', lineHeight: 1.5 }}>{f}</span>
              </div>
            ))}
          </div>
        </div>

        <span className="auth-footer-text">© 2026 NexusVoice · All rights reserved</span>
      </div>

      {/* Right panel — form */}
      <div className="auth-right">
        <div className="auth-box">
          <div>
            <h1 className="auth-title">
              {mode === 'login' ? 'Welcome back' : 'Get started'}
            </h1>
            <p className="auth-subtitle">
              {mode === 'login'
                ? 'Sign in to your NexusVoice account'
                : 'Create your free account'}
            </p>
          </div>

          {(error || errors.root) && (
            <div className="notice notice--error">
              <AlertCircle size={13} strokeWidth={2} style={{ flexShrink: 0, color: 'var(--danger)' }} />
              <span style={{ flex: 1 }}>{error ?? errors.root?.message}</span>
              <button type="button" className="notice__close" onClick={() => setError(null)}>
                <X size={13} strokeWidth={2} />
              </button>
            </div>
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
              {errors.email && <p className="auth-field-error" role="alert">{errors.email.message}</p>}
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
              {errors.password && <p className="auth-field-error" role="alert">{errors.password.message}</p>}
              {mode === 'register' && !errors.password && (
                <p className="auth-field-hint">Min. 8 chars · 1 uppercase · 1 number</p>
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
              <Label
                htmlFor="auth-remember"
                style={{ textTransform: 'none', fontSize: '11px', letterSpacing: 0, fontWeight: 400, color: 'var(--fg-2)', cursor: 'pointer' }}
              >
                Remember me for 30 days
              </Label>
            </div>

            <Button type="submit" className="w-full" disabled={isSubmitting} style={{ marginTop: '4px' }}>
              {isSubmitting
                ? (mode === 'login' ? 'Signing in…' : 'Creating account…')
                : (mode === 'login' ? 'Sign in' : 'Create account')}
            </Button>
          </form>

          <p className="auth-switch">
            {mode === 'login' ? "Don't have an account?" : 'Already have an account?'}{' '}
            <button
              type="button"
              className="auth-switch-btn"
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
