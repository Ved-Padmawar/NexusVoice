import { useState } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import { AnimatePresence, motion } from 'framer-motion'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Zap, Check, AlertCircle, X, Minus, Square } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'
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
    defaultValues: { email: '', password: '', rememberMe: false },
  })

  const onSubmit = async (data: FormValues) => {
    try {
      if (mode === 'login') await login(data.email, data.password)
      else await register(data.email, data.password)
      navigate(from, { replace: true })
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Authentication failed. Please try again.'
      setFieldError('root', { message })
    }
  }

  const switchMode = (next: Mode) => {
    setMode(next)
    reset({ email: '', password: '', rememberMe: false })
  }

  const win = getCurrentWindow()

  return (
    <div className="flex flex-col h-dvh overflow-hidden bg-[var(--bg)]">
      {/* Titlebar */}
      <div className="flex items-stretch h-8 flex-shrink-0 bg-[var(--panel)] border-b border-[var(--border)] select-none">
        <div className="flex-1 h-full cursor-default" data-tauri-drag-region />
        <div className="flex items-stretch no-drag">
          <button className="flex items-center justify-center w-[46px] h-full bg-transparent border-none cursor-pointer text-[var(--muted)] transition-[background,color] duration-[var(--t-fast)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)]" onClick={() => win.minimize()} aria-label="Minimize"><Minus size={10} strokeWidth={2} /></button>
          <button className="flex items-center justify-center w-[46px] h-full bg-transparent border-none cursor-pointer text-[var(--muted)] transition-[background,color] duration-[var(--t-fast)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)]" onClick={() => win.toggleMaximize()} aria-label="Maximize"><Square size={9} strokeWidth={2} /></button>
          <button className="flex items-center justify-center w-[46px] h-full bg-transparent border-none cursor-pointer text-[var(--muted)] transition-[background,color] duration-[var(--t-fast)] hover:bg-[#c42b1c] hover:text-white" onClick={() => win.close()} aria-label="Close"><X size={10} strokeWidth={2} /></button>
        </div>
      </div>

      <div className="flex flex-1 min-h-0">
        {/* Left panel */}
        <div className="flex-1 flex flex-col justify-between px-12 py-10 bg-[var(--panel)] border-r border-[var(--border)] relative overflow-hidden">
          {/* Ambient blobs */}
          <div className="absolute top-[-100px] left-[-60px] w-[420px] h-[420px] rounded-full pointer-events-none opacity-10" style={{ background: 'radial-gradient(circle, var(--accent) 0%, transparent 65%)' }} />
          <div className="absolute bottom-[-80px] right-[-50px] w-[300px] h-[300px] rounded-full pointer-events-none opacity-[0.06]" style={{ background: 'radial-gradient(circle, var(--accent) 0%, transparent 65%)' }} />

          {/* Brand */}
          <div className="flex items-center gap-[10px] relative z-10">
            <div className="w-9 h-9 rounded-[var(--r-lg)] bg-[var(--accent)] flex items-center justify-center text-[var(--accent-fg)] shadow-[var(--glow)]">
              <Zap size={15} strokeWidth={2.5} />
            </div>
            <span className="text-[16px] font-extrabold tracking-[-0.025em] text-[var(--fg)]">NexusVoice</span>
          </div>

          {/* Hero */}
          <div className="relative z-10">
            <h2 className="text-[30px] font-extrabold tracking-[-0.04em] text-[var(--fg)] leading-[1.12] m-0 mb-[10px]">
              Voice-to-text,<br /><span className="text-[var(--accent)]">reimagined.</span>
            </h2>
            <p className="text-[13px] text-[var(--muted)] leading-[1.6] max-w-[280px] m-0">
              Speak naturally. Paste instantly. Works everywhere on your desktop.
            </p>

            <div className="flex flex-col gap-[10px] mt-7">
              {FEATURES.map((f) => (
                <div key={f} className="flex items-start gap-[10px]">
                  <div className="w-[18px] h-[18px] rounded-[var(--r-sm)] bg-[var(--accent-soft)] text-[var(--accent)] flex items-center justify-center flex-shrink-0 mt-px">
                    <Check size={10} strokeWidth={3} />
                  </div>
                  <span className="text-[12px] text-[var(--fg-2)] leading-[1.5]">{f}</span>
                </div>
              ))}
            </div>
          </div>

          <span className="text-[10px] text-[var(--muted)] opacity-50 relative z-10">© 2026 NexusVoice · All rights reserved</span>
        </div>

        {/* Right panel — form */}
        <div className="w-[400px] flex-shrink-0 flex items-center justify-center px-9 py-10 bg-[var(--bg)]">
          <div className="w-full flex flex-col gap-5">
            <AnimatePresence mode="wait" initial={false}>
              <motion.div
                key={mode}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -8 }}
                transition={{ duration: 0.18 }}
              >
                <h1 className="text-[20px] font-bold tracking-[-0.025em] text-[var(--fg)] m-0">
                  {mode === 'login' ? 'Welcome back' : 'Get started'}
                </h1>
                <p className="text-[12px] text-[var(--muted)] mt-[3px] leading-[1.5]">
                  {mode === 'login'
                    ? 'Sign in to your NexusVoice account'
                    : 'Create your free account'}
                </p>
              </motion.div>
            </AnimatePresence>

            <AnimatePresence>
              {errors.root && (
                <motion.div key="auth-error" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.18 }} style={{ overflow: 'hidden' }}>
                  <div className="flex items-center gap-[10px] px-[14px] py-[10px] rounded-[var(--r-lg)] text-[12px] leading-[1.4] text-[var(--fg-2)]" style={{ background: 'color-mix(in srgb, var(--danger, #e05555) 12%, transparent)', border: '1px solid color-mix(in srgb, var(--danger, #e05555) 30%, transparent)' }}>
                    <AlertCircle size={13} strokeWidth={2} className="flex-shrink-0 text-[var(--danger)]" />
                    <span className="flex-1">{errors.root.message}</span>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            <form onSubmit={handleSubmit(onSubmit)} className="flex flex-col gap-3" noValidate>
              <div className="flex flex-col gap-[5px]">
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
                {errors.email && <p className="text-[11px] text-[var(--danger)] m-0" role="alert">{errors.email.message}</p>}
              </div>

              <div className="flex flex-col gap-[5px]">
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
                {errors.password && <p className="text-[11px] text-[var(--danger)] m-0" role="alert">{errors.password.message}</p>}
                {mode === 'register' && !errors.password && (
                  <p className="text-[11px] text-[var(--muted)] m-0">Min. 8 chars · 1 uppercase · 1 number</p>
                )}
              </div>

              <div className="flex items-center gap-[7px]">
                <input
                  id="auth-remember"
                  type="checkbox"
                  className="w-[14px] h-[14px] rounded-[var(--r-xs)] border-[1.5px] border-[var(--border)] bg-[var(--surface)] cursor-pointer flex-shrink-0 accent-[var(--accent)]"
                  disabled={isSubmitting}
                  {...formRegister('rememberMe')}
                />
                <Label htmlFor="auth-remember" className="text-[11px] font-normal text-[var(--fg-2)] cursor-pointer tracking-normal normal-case">
                  Remember me for 30 days
                </Label>
              </div>

              <Button type="submit" className="w-full mt-1" disabled={isSubmitting}>
                {isSubmitting
                  ? (mode === 'login' ? 'Signing in…' : 'Creating account…')
                  : (mode === 'login' ? 'Sign in' : 'Create account')}
              </Button>
            </form>

            <p className="text-[11px] text-[var(--muted)] text-center">
              {mode === 'login' ? "Don't have an account?" : 'Already have an account?'}{' '}
              <button
                type="button"
                className="bg-transparent border-none text-[var(--accent)] cursor-pointer text-[inherit] font-semibold p-0 hover:underline"
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
