import { useEffect } from 'react'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Alert, AlertDescription } from '@/components/ui/alert'

function formatSpeakingTime(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  const m = Math.floor(seconds / 60)
  const s = seconds % 60
  if (m < 60) return s > 0 ? `${m}m ${s}s` : `${m}m`
  const h = Math.floor(m / 60)
  const rm = m % 60
  return rm > 0 ? `${h}h ${rm}m` : `${h}h`
}

function formatDate(dateStr: string): string {
  try {
    return new Date(dateStr).toLocaleString(undefined, {
      month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
    })
  } catch {
    return dateStr
  }
}

export function Dashboard() {
  const {
    transcripts,
    stats,
    fetchStats,
    error,
    setError,
  } = useAppStore()

  useEffect(() => {
    fetchStats()
  }, [fetchStats])

  const handleCopy = async (content: string) => {
    await navigator.clipboard.writeText(content)
  }

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div className="page-header">
        <h1 className="page-title">Dashboard</h1>
        <p className="page-subtitle">Overview of your transcription activity.</p>
      </div>

      {error && (
        <Alert variant="destructive" className="mb-5 flex-shrink-0">
          <AlertDescription className="flex items-center justify-between">
            {error}
            <button
              type="button"
              onClick={() => setError(null)}
              aria-label="Dismiss"
              className="opacity-70 hover:opacity-100 text-base leading-none ml-2"
            >×</button>
          </AlertDescription>
        </Alert>
      )}

      {/* Stats */}
      <div className="flex gap-3.5 flex-wrap flex-shrink-0 mb-5">
        {[
          { value: stats?.totalWords.toLocaleString(), label: 'Total Words' },
          { value: stats ? formatSpeakingTime(stats.speakingTimeSeconds) : undefined, label: 'Speaking Time' },
          { value: stats?.totalSessions.toLocaleString(), label: 'Sessions' },
          { value: stats ? String(stats.avgPaceWpm) : undefined, label: 'Avg Pace (wpm)' },
        ].map(({ value, label }) => (
          <div key={label} className="stat-card flex-1 min-w-[120px]">
            <p className="stat-value">{value ?? '—'}</p>
            <p className="stat-label">{label}</p>
          </div>
        ))}
      </div>

      {/* Recent Activity */}
      <div className="card flex-1 min-h-0 flex flex-col">
        <div className="card__header">
          <div>
            <h2 className="card__title">Recent Activity</h2>
            <p className="card__desc">Your latest transcription sessions.</p>
          </div>
        </div>
        <div className="card__body overflow-y-auto flex-1 min-h-0">
          {transcripts.length === 0 ? (
            <div className="flex flex-col items-center gap-2 py-7 text-center">
              <div className="w-9 h-9 rounded-xl bg-[var(--surface)] border border-[var(--border-subtle)] flex items-center justify-center text-[var(--muted-color)] opacity-70">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M12 14c1.66 0 3-1.34 3-3V5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3z"/>
                  <path d="M17 11c0 2.76-2.24 5-5 5s-5-2.24-5-5H5c0 3.53 2.61 6.43 6 6.92V21h2v-3.08c3.39-.49 6-3.39 6-6.92h-2z"/>
                </svg>
              </div>
              <p className="text-[13px] text-[var(--muted-color)] max-w-[260px] leading-relaxed">No transcripts yet. Start recording to see activity here.</p>
            </div>
          ) : (
            <div className="flex flex-col gap-1.5">
              {transcripts.map((item) => (
                <article key={item.id} className="flex flex-col gap-2 p-3 rounded-xl bg-[var(--surface)] border border-transparent hover:border-[var(--border-subtle)] transition-colors">
                  <p className="text-[13px] text-[var(--fg)] leading-relaxed">{item.content}</p>
                  <div className="flex items-center justify-between">
                    <p className="text-[11px] text-[var(--muted-color)]">{formatDate(item.createdAt)}</p>
                    <Button variant="ghost" size="sm" type="button" onClick={() => handleCopy(item.content)} className="h-6 px-2 text-xs opacity-60 hover:opacity-100">
                      Copy
                    </Button>
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
