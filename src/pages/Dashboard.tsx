import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { AnimatePresence, motion } from 'framer-motion'
import {
  Hash, Timer, Mic, Activity,
  AlertCircle, Copy, Check, X,
  Settings2,
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
  { key: 'totalWords',         label: 'Total Words',   fmt: (v: number) => v.toLocaleString(),   Icon: Hash },
  { key: 'speakingTimeSeconds',label: 'Speaking Time', fmt: (v: number) => fmtTime(v),           Icon: Timer },
  { key: 'totalSessions',      label: 'Sessions',      fmt: (v: number) => v.toLocaleString(),   Icon: Mic },
  { key: 'avgPaceWpm',         label: 'Avg Pace',      fmt: (v: number) => `${v} wpm`,           Icon: Activity },
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
      className={`activity-copy${copied ? ' activity-copy--copied' : ''}`}
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
    <div className="dashboard">
      <div className="page-header">
        <h1 className="page-title">Dashboard</h1>
        <p className="page-subtitle">Your transcription activity at a glance.</p>
      </div>

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

      <div className="stat-grid">
        {STATS.map(({ key, label, fmt, Icon }, i) => {
          const raw = stats?.[key as keyof typeof stats] as number | undefined
          return (
            <motion.div
              key={key}
              className="stat-card"
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.2, delay: i * 0.05 }}
            >
              <div className="stat-icon"><Icon size={14} strokeWidth={1.75} /></div>
              <p className="stat-value">{raw != null ? fmt(raw) : '—'}</p>
              <p className="stat-label">{label}</p>
            </motion.div>
          )
        })}
      </div>

      <div className="card dashboard__activity">
        <div className="card__header">
          <div>
            <h2 className="card__title">Recent Activity</h2>
            <p className="card__desc">
              {transcripts.length > 0
                ? `${transcripts.length} transcript${transcripts.length !== 1 ? 's' : ''}`
                : 'No transcripts yet'}
            </p>
          </div>
        </div>
        <div className="card__body dashboard__activity-body">
          {transcripts.length === 0 ? (
            <div className="empty-state">
              <div className="empty-icon"><Mic size={16} strokeWidth={1.5} /></div>
              <p className="empty-text">Hold your hotkey and speak. Transcripts appear here automatically.</p>
            </div>
          ) : (
            <div className="stack-sm">
              <AnimatePresence initial={false}>
                {transcripts.map((item) => (
                  <motion.article
                    key={item.id}
                    className="activity-item"
                    initial={{ opacity: 0, y: -8 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, height: 0 }}
                    transition={{ duration: 0.18 }}
                    layout
                  >
                    <p className="activity-text">{item.content}</p>
                    <div className="activity-meta">
                      <span className="activity-time">{fmtDate(item.createdAt)}</span>
                      <CopyButton text={item.content} />
                    </div>
                  </motion.article>
                ))}
              </AnimatePresence>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

