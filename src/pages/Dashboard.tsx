import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { AnimatePresence, motion } from 'framer-motion'
import {
  Hash, Timer, Mic, Activity,
  AlertCircle, Copy, Check, X,
  Settings2, Zap,
} from 'lucide-react'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'

function fmtTime(s: number) {
  if (s < 60) return `${s}s`
  const m = Math.floor(s / 60), rem = s % 60
  if (m < 60) return rem > 0 ? `${m}m ${rem}s` : `${m}m`
  const h = Math.floor(m / 60), rm = m % 60
  return rm > 0 ? `${h}h ${rm}m` : `${h}h`
}

function fmtDate(d: string) {
  try {
    return new Date(d).toLocaleString(undefined, {
      month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
    })
  } catch { return d }
}

const STATS = [
  { key: 'totalWords',          label: 'Total Words',   fmt: (v: number) => v.toLocaleString(),   Icon: Hash },
  { key: 'speakingTimeSeconds', label: 'Speaking Time', fmt: (v: number) => fmtTime(v),           Icon: Timer },
  { key: 'totalSessions',       label: 'Sessions',      fmt: (v: number) => v.toLocaleString(),   Icon: Mic },
  { key: 'avgPaceWpm',          label: 'Avg Pace',      fmt: (v: number) => `${v}`,               Icon: Activity },
]

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)
  const handleCopy = () => {
    navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }
  return (
    <button
      type="button"
      className={`inline-flex items-center gap-1 bg-transparent border-none cursor-pointer text-[10px] font-medium px-[6px] py-[2px] rounded-[var(--r-sm)] tracking-[0.02em] transition-colors duration-[var(--t-fast)] ${copied ? 'text-[var(--success)]' : 'text-[var(--muted)] hover:text-[var(--accent)]'}`}
      onClick={handleCopy}
      title="Copy to clipboard"
    >
      {copied ? <Check size={11} strokeWidth={2.5} /> : <Copy size={11} strokeWidth={2} />}
      {copied ? 'Copied' : 'Copy'}
    </button>
  )
}

export function Dashboard() {
  const { transcripts, stats, hasHotkey, error, setError } = useAppStore()
  const navigate = useNavigate()

  return (
    <div className="flex flex-col h-full overflow-hidden px-8 pt-7 pb-10 gap-7">

      {/* Hero */}
      <div className="flex items-center justify-between gap-4 pb-5 border-b border-[var(--border-soft)]">
        <div className="flex items-center gap-[14px]">
          <div className="w-9 h-9 rounded-[var(--r-lg)] bg-[var(--accent-soft)] text-[var(--accent)] flex items-center justify-center flex-shrink-0">
            <Zap size={18} strokeWidth={2.5} />
          </div>
          <div>
            <h1 className="text-[18px] font-bold tracking-[-0.025em] text-[var(--fg)] leading-[1.1] m-0">Dashboard</h1>
            <p className="text-[12px] text-[var(--muted)] mt-[3px] m-0">Your voice, transcribed instantly.</p>
          </div>
        </div>
      </div>

      {/* Notices */}
      <AnimatePresence>
        {!hasHotkey && (
          <motion.div key="hotkey-notice" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
            <div className="flex items-center gap-[10px] px-[14px] py-[10px] rounded-[var(--r-lg)] text-[12px] leading-[1.4] flex-shrink-0 text-[var(--fg-2)]" style={{ background: 'var(--warning-soft)', border: '1px solid oklch(from var(--warning) l c h / 0.25)' }}>
              <AlertCircle size={14} strokeWidth={2} className="flex-shrink-0 text-[var(--warning)]" />
              <span className="flex-1">No hotkey set — NexusVoice won't record until you configure one.</span>
              <Button size="sm" onClick={() => navigate('/settings', { state: { tab: 'general' } })} className="flex-shrink-0">
                <Settings2 size={12} strokeWidth={2} />
                Set hotkey
              </Button>
            </div>
          </motion.div>
        )}
        {error && (
          <motion.div key="error-notice" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
            <div className="flex items-center gap-[10px] px-[14px] py-[10px] rounded-[var(--r-lg)] text-[12px] leading-[1.4] flex-shrink-0 text-[var(--fg-2)]" style={{ background: 'var(--danger-soft)', border: '1px solid oklch(from var(--danger) l c h / 0.30)' }}>
              <AlertCircle size={14} strokeWidth={2} className="flex-shrink-0 text-[var(--danger)]" />
              <span className="flex-1">{error}</span>
              <button type="button" className="ml-auto text-[var(--muted)] bg-transparent border-none cursor-pointer px-[2px] text-[15px] leading-none rounded-[var(--r-xs)] flex-shrink-0 opacity-60 hover:opacity-100 transition-opacity" onClick={() => setError(null)}>
                <X size={14} strokeWidth={2} />
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Stats */}
      <div className="grid grid-cols-4 gap-[10px]">
        {STATS.map(({ key, label, fmt, Icon }, i) => {
          const raw = stats?.[key as keyof typeof stats] as number | undefined
          return (
            <motion.div
              key={key}
              className="flex items-center gap-[14px] px-[18px] py-4 rounded-[var(--r-xl)] bg-[var(--panel)] border border-[var(--border)] cursor-default"
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.22, delay: i * 0.06 }}
            >
              <div className="w-9 h-9 rounded-[var(--r-md)] bg-[var(--accent-soft)] text-[var(--accent)] flex items-center justify-center flex-shrink-0">
                <Icon size={15} strokeWidth={1.75} />
              </div>
              <div className="flex flex-col gap-[3px]">
                <span className="text-[20px] font-bold tracking-[-0.03em] text-[var(--fg)] leading-none tabular-nums">{raw != null ? fmt(raw) : '—'}</span>
                <span className="text-[11px] text-[var(--muted)] font-medium">{label}</span>
              </div>
            </motion.div>
          )
        })}
      </div>

      {/* Activity feed */}
      <div className="flex flex-col flex-1 min-h-0 overflow-hidden">
        <div className="flex items-center gap-[10px] mb-5">
          <h2 className="text-[13px] font-bold text-[var(--fg)] tracking-[-0.01em] m-0">Recent Activity</h2>
          {transcripts.length > 0 && (
            <span className="inline-flex items-center justify-center min-w-5 h-5 px-[6px] rounded-full bg-[var(--accent-soft)] text-[var(--accent)] text-[10px] font-bold tracking-[0.02em]">
              {transcripts.length}
            </span>
          )}
        </div>

        {transcripts.length === 0 ? (
          <div className="flex flex-col items-center gap-3 py-14 px-6 text-center">
            <div className="w-14 h-14 rounded-full border-[1.5px] border-dashed border-[var(--border)] flex items-center justify-center text-[var(--muted)]">
              <Mic size={20} strokeWidth={1.5} />
            </div>
            <p className="text-[13px] font-semibold text-[var(--fg-2)] m-0">Nothing here yet</p>
            <p className="text-[12px] text-[var(--muted)] max-w-[260px] leading-[1.6] m-0">Hold your hotkey and speak — transcripts stream in automatically.</p>
          </div>
        ) : (
          <div className="flex flex-col gap-0 overflow-y-auto overflow-x-hidden flex-1 min-h-0 pr-[6px]">
            <AnimatePresence initial={false}>
              {transcripts.map((item) => (
                <motion.article
                  key={item.id}
                  className="grid grid-cols-[20px_1fr] gap-x-[14px] relative pb-4"
                  initial={{ opacity: 0, y: -6 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.18 }}
                  layout
                >
                  {/* Timeline line */}
                  <div className="absolute left-[9px] top-[22px] bottom-[-16px] w-px bg-[var(--border-soft)] last:hidden" aria-hidden />
                  {/* Dot */}
                  <div className="col-start-1 row-start-1 w-2 h-2 rounded-full bg-[var(--accent)] mt-3 justify-self-center relative z-10 flex-shrink-0" aria-hidden />
                  {/* Card */}
                  <div className="col-start-2 row-start-1 bg-[var(--panel)] border border-[var(--border-soft)] rounded-[var(--r-lg)] px-[14px] py-3 flex flex-col gap-2 transition-[border-color,background] duration-[var(--t-fast)] hover:border-[var(--border)] hover:bg-[var(--surface)]">
                    <p className="text-[13px] text-[var(--fg)] leading-[1.6] m-0">{item.content}</p>
                    <div className="flex items-center justify-between">
                      <span className="text-[10px] text-[var(--muted)] tabular-nums">{fmtDate(item.createdAt)}</span>
                      <CopyButton text={item.content} />
                    </div>
                  </div>
                </motion.article>
              ))}
            </AnimatePresence>
          </div>
        )}
      </div>

    </div>
  )
}
