import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  Hash, Timer, Mic, Activity,
  AlertCircle, Copy, Check,
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
      className="activity-copy"
      onClick={handleCopy}
      title="Copy to clipboard"
      style={{ color: copied ? 'var(--success)' : undefined, transition: 'color 0.15s' }}
    >
      {copied ? <Check size={11} strokeWidth={2.5} /> : <Copy size={11} strokeWidth={2} />}
      {copied ? 'Copied' : 'Copy'}
    </button>
  )
}

export function Dashboard() {
  const { transcripts, stats, fetchStats, hasHotkey, error, setError } = useAppStore()
  const navigate = useNavigate()

  useEffect(() => { fetchStats() }, [fetchStats])

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden', gap: '16px' }}>
      {/* Header */}
      <div className="page-header" style={{ marginBottom: 0 }}>
        <h1 className="page-title">Dashboard</h1>
        <p className="page-subtitle">Your transcription activity at a glance.</p>
      </div>

      {/* Banners */}
      {!hasHotkey && (
        <div className="notice notice--warning">
          <AlertCircle size={14} strokeWidth={2} style={{ flexShrink: 0, color: 'var(--warning)' }} />
          <span style={{ flex: 1 }}>No hotkey set — NexusVoice won't record until you configure one.</span>
          <Button
            size="sm"
            onClick={() => navigate('/settings', { state: { tab: 'audio' } })}
            style={{ flexShrink: 0 }}
          >
            <Settings2 size={12} strokeWidth={2} />
            Set hotkey
          </Button>
        </div>
      )}

      {error && (
        <div className="notice notice--error">
          <AlertCircle size={14} strokeWidth={2} style={{ flexShrink: 0, color: 'var(--danger)' }} />
          <span style={{ flex: 1 }}>{error}</span>
          <button type="button" className="notice__close" onClick={() => setError(null)}>
            <X size={14} strokeWidth={2} />
          </button>
        </div>
      )}

      {/* Stats */}
      <div className="stat-grid">
        {STATS.map(({ key, label, fmt, Icon }) => {
          const raw = stats?.[key as keyof typeof stats] as number | undefined
          return (
            <div key={key} className="stat-card">
              <div className="stat-icon"><Icon size={14} strokeWidth={1.75} /></div>
              <p className="stat-value">{raw != null ? fmt(raw) : '—'}</p>
              <p className="stat-label">{label}</p>
            </div>
          )
        })}
      </div>

      {/* Activity */}
      <div className="card" style={{ flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column' }}>
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

        <div className="card__body" style={{ flex: 1, minHeight: 0, overflowY: 'auto' }}>
          {transcripts.length === 0 ? (
            <div className="empty-state">
              <div className="empty-icon">
                <Mic size={16} strokeWidth={1.5} />
              </div>
              <p className="empty-text">
                Hold your hotkey and speak. Transcripts appear here automatically.
              </p>
            </div>
          ) : (
            <div className="stack-sm">
              {transcripts.map((item) => (
                <article key={item.id} className="activity-item">
                  <p className="activity-text">{item.content}</p>
                  <div className="activity-meta">
                    <span className="activity-time">{fmtDate(item.createdAt)}</span>
                    <CopyButton text={item.content} />
                  </div>
                </article>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

// X icon inline (not re-exporting from lucide to avoid import clash)
function X({ size, strokeWidth }: { size: number; strokeWidth: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={strokeWidth} strokeLinecap="round" strokeLinejoin="round">
      <line x1="18" y1="6" x2="6" y2="18"/>
      <line x1="6" y1="6" x2="18" y2="18"/>
    </svg>
  )
}
