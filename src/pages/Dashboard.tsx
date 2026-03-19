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
  { key: 'totalWords',          label: 'Total Words',   sublabel: 'transcribed',    fmt: (v: number) => v.toLocaleString(),   Icon: Hash },
  { key: 'speakingTimeSeconds', label: 'Speaking Time', sublabel: 'recorded',       fmt: (v: number) => fmtTime(v),           Icon: Timer },
  { key: 'totalSessions',       label: 'Sessions',      sublabel: 'completed',      fmt: (v: number) => v.toLocaleString(),   Icon: Mic },
  { key: 'avgPaceWpm',          label: 'Avg Pace',      sublabel: 'words / min',    fmt: (v: number) => `${v}`,               Icon: Activity },
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
      className={`db-copy${copied ? ' db-copy--done' : ''}`}
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
    <div className="db">

      {/* ── Hero ─────────────────────────────────────────────────── */}
      <div className="db__hero">
        <div className="db__hero-glow" aria-hidden />
        <div className="db__hero-body">
          <div className="db__hero-icon">
            <Zap size={18} strokeWidth={2.5} />
          </div>
          <div>
            <h1 className="db__hero-title">Dashboard</h1>
            <p className="db__hero-sub">Your voice, transcribed instantly.</p>
          </div>
        </div>
      </div>

      {/* ── Notices ──────────────────────────────────────────────── */}
      <AnimatePresence>
        {!hasHotkey && (
          <motion.div key="hotkey-notice" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
            <div className="notice notice--warning">
              <AlertCircle size={14} strokeWidth={2} className="icon--shrink icon--warning" />
              <span className="text--flex">No hotkey set — NexusVoice won't record until you configure one.</span>
              <Button size="sm" onClick={() => navigate('/settings', { state: { tab: 'audio' } })} className="icon--shrink">
                <Settings2 size={12} strokeWidth={2} />
                Set hotkey
              </Button>
            </div>
          </motion.div>
        )}
        {error && (
          <motion.div key="error-notice" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
            <div className="notice notice--error">
              <AlertCircle size={14} strokeWidth={2} className="icon--shrink icon--danger" />
              <span className="text--flex">{error}</span>
              <button type="button" className="notice__close" onClick={() => setError(null)}>
                <X size={14} strokeWidth={2} />
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* ── Stats ────────────────────────────────────────────────── */}
      <div className="db__stats">
        {STATS.map(({ key, label, sublabel, fmt, Icon }, i) => {
          const raw = stats?.[key as keyof typeof stats] as number | undefined
          return (
            <motion.div
              key={key}
              className="db__stat"
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.22, delay: i * 0.06 }}
            >
              <div className="db__stat-icon">
                <Icon size={15} strokeWidth={1.75} />
              </div>
              <div className="db__stat-body">
                <span className="db__stat-value">{raw != null ? fmt(raw) : '—'}</span>
                <span className="db__stat-label">{label}</span>
              </div>
              <span className="db__stat-sub">{sublabel}</span>
            </motion.div>
          )
        })}
      </div>

      {/* ── Activity feed ────────────────────────────────────────── */}
      <div className="db__feed">
        <div className="db__feed-header">
          <h2 className="db__feed-title">Recent Activity</h2>
          {transcripts.length > 0 && (
            <span className="db__feed-count">{transcripts.length}</span>
          )}
        </div>

        {transcripts.length === 0 ? (
          <div className="db__empty">
            <div className="db__empty-ring">
              <Mic size={20} strokeWidth={1.5} />
            </div>
            <p className="db__empty-title">Nothing here yet</p>
            <p className="db__empty-hint">Hold your hotkey and speak — transcripts stream in automatically.</p>
          </div>
        ) : (
          <div className="db__feed-list">
            <AnimatePresence initial={false}>
              {transcripts.map((item) => (
                <motion.article
                  key={item.id}
                  className="db__entry"
                  initial={{ opacity: 0, y: -6 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.18 }}
                  layout
                >
                  <div className="db__entry-line" aria-hidden />
                  <div className="db__entry-dot" aria-hidden />
                  <div className="db__entry-card">
                    <p className="db__entry-text">{item.content}</p>
                    <div className="db__entry-footer">
                      <span className="db__entry-time">{fmtDate(item.createdAt)}</span>
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
