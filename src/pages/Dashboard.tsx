import { useEffect, useState } from 'react'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'

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
    dictionary,
    stats,
    fetchStats,
    updateDictionary,
    error,
    setError,
  } = useAppStore()

  const [term, setTerm] = useState('')
  const [replacement, setReplacement] = useState('')
  const [addTarget, setAddTarget] = useState<string | null>(null)

  useEffect(() => {
    fetchStats()
  }, [fetchStats])

  const handleCopy = async (content: string) => {
    await navigator.clipboard.writeText(content)
  }

  const handleAddToDictionary = (word: string) => {
    setAddTarget(word)
    setTerm(word)
    setReplacement('')
  }

  const handleDictionarySubmit = () => {
    if (!term.trim() || !replacement.trim()) return
    updateDictionary(term.trim(), replacement.trim())
    setTerm('')
    setReplacement('')
    setAddTarget(null)
  }

  return (
    <div className="settings-page">
      <div className="page-header">
        <h1 className="page-title">Dashboard</h1>
        <p className="page-subtitle">Overview of your transcription activity.</p>
      </div>

      {error && (
        <Alert variant="destructive" className="mb-4">
          <AlertDescription className="flex items-center justify-between">
            {error}
            <button
              type="button"
              onClick={() => setError(null)}
              aria-label="Dismiss"
              className="opacity-70 hover:opacity-100 text-base leading-none ml-2"
            >
              ×
            </button>
          </AlertDescription>
        </Alert>
      )}

      <Tabs defaultValue="overview" className="settings-tabs">
        <TabsList className="settings-tabs-list">
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="dictionary">Dictionary</TabsTrigger>
        </TabsList>

        {/* ===== Overview Tab ===== */}
        <TabsContent value="overview" className="settings-tab-content">
          {/* Stats */}
          <div className="stat-row" style={{ marginBottom: 20 }}>
            <div className="stat-card">
              <p className="stat-value">{stats ? stats.totalWords.toLocaleString() : '—'}</p>
              <p className="stat-label">Total Words</p>
            </div>
            <div className="stat-card">
              <p className="stat-value">{stats ? formatSpeakingTime(stats.speakingTimeSeconds) : '—'}</p>
              <p className="stat-label">Speaking Time</p>
            </div>
            <div className="stat-card">
              <p className="stat-value">{stats ? stats.totalSessions.toLocaleString() : '—'}</p>
              <p className="stat-label">Sessions</p>
            </div>
            <div className="stat-card">
              <p className="stat-value">{stats ? `${stats.avgPaceWpm}` : '—'}</p>
              <p className="stat-label">Avg Pace (wpm)</p>
            </div>
          </div>

          {/* Recent Activity */}
          <div className="card">
            <div className="card__header">
              <div>
                <h2 className="card__title">Recent Activity</h2>
                <p className="card__desc">Your latest transcription sessions.</p>
              </div>
            </div>
            <div className="card__body">
              {transcripts.length === 0 ? (
                <div className="empty-state">
                  <div className="empty-state-icon">
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M12 14c1.66 0 3-1.34 3-3V5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3z"/>
                      <path d="M17 11c0 2.76-2.24 5-5 5s-5-2.24-5-5H5c0 3.53 2.61 6.43 6 6.92V21h2v-3.08c3.39-.49 6-3.39 6-6.92h-2z"/>
                    </svg>
                  </div>
                  <p className="empty-state-text">No transcripts yet. Start recording to see activity here.</p>
                </div>
              ) : (
                <div className="list">
                  {transcripts.map((item) => (
                    <article key={item.id} className="list-item list-item--block">
                      <div className="list-item__body">
                        <p className="list-text">{item.content}</p>
                        <p className="list-meta">{formatDate(item.createdAt)}</p>
                      </div>
                      <div className="list-item__actions">
                        <Button
                          variant="ghost"
                          size="sm"
                          type="button"
                          onClick={() => handleCopy(item.content)}
                          title="Copy"
                          className="h-7 px-2 text-xs"
                        >
                          Copy
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          type="button"
                          onClick={() => handleAddToDictionary(item.content.split(' ')[0])}
                          className="h-7 px-2 text-xs"
                        >
                          + Dictionary
                        </Button>
                      </div>
                    </article>
                  ))}
                </div>
              )}
            </div>
          </div>
        </TabsContent>

        {/* ===== Dictionary Tab ===== */}
        <TabsContent value="dictionary" className="settings-tab-content">
          <div className="card" style={{ marginBottom: 16 }}>
            <div className="card__header">
              <div>
                <h2 className="card__title">Add Entry</h2>
                <p className="card__desc">Map a misheard word to its correct replacement.</p>
              </div>
              {addTarget && (
                <Badge variant="secondary">Pre-filled from transcript</Badge>
              )}
            </div>
            <div className="card__body">
              <div className="dictionary-form">
                <div className="field">
                  <Label htmlFor="dict-term">Term</Label>
                  <Input
                    id="dict-term"
                    value={term}
                    onChange={(e) => setTerm(e.target.value)}
                    placeholder="recieve"
                  />
                </div>
                <div className="field">
                  <Label htmlFor="dict-replace">Replacement</Label>
                  <Input
                    id="dict-replace"
                    value={replacement}
                    onChange={(e) => setReplacement(e.target.value)}
                    placeholder="receive"
                  />
                </div>
                <Button
                  size="sm"
                  type="button"
                  onClick={handleDictionarySubmit}
                  disabled={!term.trim() || !replacement.trim()}
                  className="self-end"
                >
                  Add
                </Button>
              </div>
            </div>
          </div>

          <div className="card">
            <div className="card__header">
              <div>
                <h2 className="card__title">Dictionary</h2>
                <p className="card__desc">{dictionary.length} {dictionary.length === 1 ? 'entry' : 'entries'}</p>
              </div>
            </div>
            <div className="card__body">
              {dictionary.length === 0 ? (
                <div className="empty-state">
                  <div className="empty-state-icon">
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M4 19.5A2.5 2.5 0 016.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 014 19.5v-15A2.5 2.5 0 016.5 2z"/>
                    </svg>
                  </div>
                  <p className="empty-state-text">No entries yet. Add words above or use "+ Dictionary" from a transcript.</p>
                </div>
              ) : (
                <div className="list">
                  {dictionary.map((entry) => (
                    <article key={entry.id} className="list-item">
                      <div style={{ flex: 1, minWidth: 0 }}>
                        <p className="list-text">
                          <Badge variant="secondary" className="mr-2">{entry.term}</Badge>
                          → {entry.replacement}
                        </p>
                        <p className="list-meta">{formatDate(entry.createdAt)}</p>
                      </div>
                      <Button
                        variant="ghost"
                        size="icon"
                        type="button"
                        onClick={() => handleCopy(entry.replacement)}
                        title="Copy replacement"
                        className="h-7 w-7"
                      >
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                          <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/>
                        </svg>
                      </Button>
                    </article>
                  ))}
                </div>
              )}
            </div>
          </div>
        </TabsContent>
      </Tabs>
    </div>
  )
}
