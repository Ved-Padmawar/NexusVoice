import { useState } from 'react'
import { useNavigate, useLocation } from 'react-router'
import { AnimatePresence, motion } from 'framer-motion'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Check, AlertCircle, X, Minus, Square, Eye, EyeOff } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import logoUrl from '../assets/logo.png'
import { extractErrorMessage } from '../lib/errors'

type Mode = 'login' | 'register'

const loginSchema = z.object({
  email: z.email('Enter a valid email address'),
  password: z.string().min(1, 'Password is required'),
})

const registerSchema = z.object({
  email: z.email('Enter a valid email address'),
  password: z
    .string()
    .min(8, 'At least 8 characters')
    .regex(/[A-Z]/, 'One uppercase letter required')
    .regex(/[0-9]/, 'One number required'),
})

type FormValues = z.infer<typeof registerSchema>

const FEATURES = [
  'Hold a hotkey, speak naturally, release to paste',
  'On-device Whisper AI — no data leaves your machine',
  'Custom dictionary for technical terms & names',
]

export function Auth() {
  const [mode, setMode] = useState<Mode>('login')
  const [showPassword, setShowPassword] = useState(false)
  const { login, register } = useAppStore()
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
    defaultValues: { email: '', password: '' },
  })

  const onSubmit = async (data: FormValues) => {
    try {
      if (mode === 'login') await login(data.email, data.password)
      else await register(data.email, data.password)
      navigate(from, { replace: true })
    } catch (e) {
      const message = extractErrorMessage(e, 'Authentication failed. Please try again.')
      setFieldError('root', { message })
    }
  }

  const switchMode = (next: Mode) => {
    setMode(next)
    setShowPassword(false)
    reset({ email: '', password: '' })
  }

  const win = getCurrentWindow()

  return (
    <div className="relative flex flex-col h-dvh overflow-hidden bg-background">
      {/* Titlebar */}
      <div className="absolute top-0 left-0 right-0 flex items-stretch h-8 z-20 select-none pointer-events-none">
        <div className="flex-1 h-full cursor-default pointer-events-auto" data-tauri-drag-region />
        <div className="flex items-stretch no-drag pointer-events-auto">
          <button className="flex items-center justify-center w-11.5 h-full bg-transparent border-none cursor-pointer text-muted-foreground transition-[background,color] duration-(--t-fast) hover:bg-accent hover:text-(--fg)" onClick={() => win.minimize()} aria-label="Minimize"><Minus size={10} strokeWidth={2} /></button>
          <button className="flex items-center justify-center w-11.5 h-full bg-transparent border-none cursor-pointer text-muted-foreground transition-[background,color] duration-(--t-fast) hover:bg-accent hover:text-(--fg)" onClick={() => win.toggleMaximize()} aria-label="Maximize"><Square size={9} strokeWidth={2} /></button>
          <button className="flex items-center justify-center w-11.5 h-full bg-transparent border-none cursor-pointer text-muted-foreground transition-[background,color] duration-(--t-fast) hover:bg-(--color-close) hover:text-white" onClick={() => win.close()} aria-label="Close"><X size={10} strokeWidth={2} /></button>
        </div>
      </div>

      <div className="flex flex-1 min-h-0">
        {/* Left panel */}
        <div className="flex-1 flex flex-col justify-between px-12 pt-9 pb-10 bg-(--panel) border-r border-(--border) relative overflow-hidden">
          {/* Ambient blobs */}
          <div className="absolute -top-25 -left-15 w-105 h-105 rounded-full pointer-events-none opacity-10" style={{ background: 'radial-gradient(circle, var(--accent) 0%, transparent 65%)' }} />
          <div className="absolute -bottom-20 -right-12.5 w-75 h-75 rounded-full pointer-events-none opacity-[0.06]" style={{ background: 'radial-gradient(circle, var(--accent) 0%, transparent 65%)' }} />

          {/* Brand */}
          <div className="flex items-center gap-2.5 relative z-10">
            <img src={logoUrl} alt="NexusVoice" className="w-9 h-9 rounded-(--r-lg) shrink-0" />
            <span className="text-[16px] font-extrabold tracking-tight text-(--fg)">NexusVoice</span>
          </div>

          {/* Hero */}
          <div className="relative z-10">
            <h2 className="text-[30px] font-extrabold tracking-[-0.04em] text-(--fg) leading-[1.12] m-0 mb-2.5">
              Voice-to-text,<br /><span className="text-(--accent)">reimagined.</span>
            </h2>
            <p className="text-[13px] text-muted-foreground leading-[1.6] max-w-70 m-0">
              Speak naturally. Paste instantly. Works everywhere on your desktop.
            </p>

            <div className="flex flex-col gap-2.5 mt-7">
              {FEATURES.map((f) => (
                <div key={f} className="flex items-start gap-2.5">
                  <div className="w-4.5 h-4.5 rounded-(--r-sm) bg-(--accent-soft) text-(--accent) flex items-center justify-center shrink-0 mt-px">
                    <Check size={10} strokeWidth={3} />
                  </div>
                  <span className="text-[12px] text-(--fg-2) leading-normal">{f}</span>
                </div>
              ))}
            </div>
          </div>

          <span className="text-[10px] text-muted-foreground opacity-50 relative z-10">© 2026 NexusVoice · All rights reserved</span>
        </div>

        {/* Right panel — form */}
        <div className="w-100 shrink-0 flex items-center justify-center px-9 pt-9 pb-10 bg-background">
          <div className="w-full flex flex-col gap-5">
            <AnimatePresence mode="wait" initial={false}>
              <motion.div
                key={mode}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -8 }}
                transition={{ duration: 0.18 }}
              >
                <h1 className="text-[20px] font-bold tracking-tight text-(--fg) m-0">
                  {mode === 'login' ? 'Welcome back' : 'Get started'}
                </h1>
                <p className="text-[12px] text-muted-foreground mt-0.75 leading-normal">
                  {mode === 'login'
                    ? 'Sign in to your NexusVoice account'
                    : 'Create your free account'}
                </p>
              </motion.div>
            </AnimatePresence>

            <AnimatePresence>
              {errors.root && (
                <motion.div key="auth-error" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.18 }} style={{ overflow: 'hidden' }}>
                  <div className="flex items-center gap-2.5 px-3.5 py-2.5 rounded-(--r-lg) text-[12px] leading-[1.4] text-(--fg-2)" style={{ background: 'var(--danger-soft)', border: '1px solid color-mix(in srgb, var(--danger) 30%, transparent)' }}>
                    <AlertCircle size={13} strokeWidth={2} className="shrink-0 text-destructive" />
                    <span className="flex-1">{errors.root.message}</span>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            <form onSubmit={handleSubmit(onSubmit)} className="flex flex-col gap-3" noValidate>
              <div className="flex flex-col gap-1.25">
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
                {errors.email && <p className="text-[11px] text-destructive m-0" role="alert">{errors.email.message}</p>}
              </div>

              <div className="flex flex-col gap-1.25">
                <Label htmlFor="auth-password">Password</Label>
                <div className="relative">
                  <Input
                    id="auth-password"
                    type={showPassword ? 'text' : 'password'}
                    placeholder="••••••••"
                    autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
                    disabled={isSubmitting}
                    aria-invalid={!!errors.password}
                    className="pr-9"
                    {...formRegister('password')}
                  />
                  <button
                    type="button"
                    onClick={() => setShowPassword(v => !v)}
                    className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-(--fg-2) transition-colors duration-(--t-fast) bg-transparent border-none cursor-pointer p-0"
                    tabIndex={-1}
                    aria-label={showPassword ? 'Hide password' : 'Show password'}
                  >
                    {showPassword ? <EyeOff size={14} strokeWidth={2} /> : <Eye size={14} strokeWidth={2} />}
                  </button>
                </div>
                {errors.password && <p className="text-[11px] text-destructive m-0" role="alert">{errors.password.message}</p>}
                {mode === 'register' && !errors.password && (
                  <p className="text-[11px] text-muted-foreground m-0">Min. 8 chars · 1 uppercase · 1 number</p>
                )}
              </div>

              <Button type="submit" className="w-full mt-1" disabled={isSubmitting}>
                {isSubmitting
                  ? (mode === 'login' ? 'Signing in…' : 'Creating account…')
                  : (mode === 'login' ? 'Sign in' : 'Create account')}
              </Button>
            </form>

            <p className="text-[11px] text-muted-foreground text-center">
              {mode === 'login' ? "Don't have an account?" : 'Already have an account?'}{' '}
              <button
                type="button"
                className="bg-transparent border-none text-(--accent) cursor-pointer font-semibold p-0 hover:underline"
                onClick={() => switchMode(mode === 'login' ? 'register' : 'login')}
              >
                {mode === 'login' ? 'Create one' : 'Sign in'}
              </button>
            </p>
          </div>
        </div>
      </div>
    </div>
  )
}
