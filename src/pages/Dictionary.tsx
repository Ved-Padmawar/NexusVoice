import { useState, useEffect } from 'react'
import { ArrowRight, Trash2, BookOpen, CheckCircle2, AlertCircle, X, Sparkles } from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

type WordSuggestion = { word: string; count: number }

export function Dictionary() {
  const { dictionary, updateDictionary, deleteDictionaryEntry, error, setError } = useAppStore()
  const [term, setTerm] = useState('')
  const [replacement, setReplacement] = useState('')
  const [saving, setSaving] = useState(false)
  const [success, setSuccess] = useState(false)
  const [suggestions, setSuggestions] = useState<WordSuggestion[]>([])

  useEffect(() => {
    invoke<WordSuggestion[]>('get_word_suggestions').then(setSuggestions).catch(() => {})
  }, [dictionary])

  const handleDismiss = async (word: string) => {
    await invoke('dismiss_word_suggestion', { word })
    setSuggestions(s => s.filter(x => x.word !== word))
  }

  const handleAdd = async () => {
    const t = term.trim(), r = replacement.trim()
    if (!t || !r) return
    setSaving(true); setError(null); setSuccess(false)
    try {
      await updateDictionary(t, r)
      setTerm(''); setReplacement('')
      setSuccess(true); setTimeout(() => setSuccess(false), 2500)
    } finally { setSaving(false) }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden', gap: '14px' }}>
      {/* Header */}
      <div className="page-header" style={{ marginBottom: 0 }}>
        <h1 className="page-title">Dictionary</h1>
        <p className="page-subtitle">
          Map spoken words to their correct form — applied automatically during transcription.
        </p>
      </div>

      {/* Banners */}
      {error && (
        <div className="notice notice--error">
          <AlertCircle size={13} strokeWidth={2} style={{ flexShrink: 0, color: 'var(--danger)' }} />
          <span style={{ flex: 1 }}>{error}</span>
          <button type="button" className="notice__close" onClick={() => setError(null)}>
            <X size={13} strokeWidth={2} />
          </button>
        </div>
      )}
      {success && (
        <div className="notice notice--success">
          <CheckCircle2 size={13} strokeWidth={2} style={{ flexShrink: 0, color: 'var(--success)' }} />
          <span>Entry saved successfully.</span>
        </div>
      )}

      {/* Add form */}
      <div className="card" style={{ flexShrink: 0 }}>
        <div className="card__header">
          <div>
            <h2 className="card__title">Add / Update Entry</h2>
            <p className="card__desc">Fuzzy matching corrects near-misses within 2 characters.</p>
          </div>
        </div>
        <div className="card__body">
          <div style={{ display: 'flex', alignItems: 'flex-end', gap: '10px' }}>
            <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: '5px' }}>
              <Label htmlFor="dict-term">Spoken / misspelled</Label>
              <Input
                id="dict-term"
                value={term}
                onChange={(e) => setTerm(e.target.value)}
                placeholder="e.g. teh, gonna"
                disabled={saving}
              />
            </div>

            <div style={{
              width: '28px', height: '36px',
              display: 'flex', alignItems: 'center', justifyContent: 'center',
              color: 'var(--muted)', flexShrink: 0,
            }}>
              <ArrowRight size={14} strokeWidth={1.75} />
            </div>

            <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: '5px' }}>
              <Label htmlFor="dict-replacement">Replace with</Label>
              <Input
                id="dict-replacement"
                value={replacement}
                onChange={(e) => setReplacement(e.target.value)}
                placeholder="e.g. the, going to"
                disabled={saving}
                onKeyDown={(e) => { if (e.key === 'Enter') handleAdd() }}
              />
            </div>

            <Button
              type="button"
              onClick={handleAdd}
              disabled={saving || !term.trim() || !replacement.trim()}
              style={{ flexShrink: 0 }}
            >
              {saving ? 'Saving…' : 'Add'}
            </Button>
          </div>
        </div>
      </div>

      {/* Auto-learn suggestions */}
      {suggestions.length > 0 && (
        <div className="card" style={{ flexShrink: 0 }}>
          <div className="card__header">
            <div>
              <h2 className="card__title" style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                <Sparkles size={13} strokeWidth={1.75} style={{ color: 'var(--accent)' }} />
                Suggested Words
              </h2>
              <p className="card__desc">Words seen 3+ times — auto-added to dictionary. Dismiss to ignore.</p>
            </div>
          </div>
          <div className="card__body">
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px' }}>
              {suggestions.map((s) => (
                <div key={s.word} style={{
                  display: 'inline-flex', alignItems: 'center', gap: '4px',
                  padding: '3px 8px', borderRadius: 'var(--r-full)',
                  background: 'var(--surface)', border: '1px solid var(--border)',
                  fontSize: '12px', color: 'var(--fg)',
                }}>
                  <span>{s.word}</span>
                  <span style={{ fontSize: '10px', color: 'var(--muted)' }}>×{s.count}</span>
                  <button type="button" onClick={() => handleDismiss(s.word)}
                    style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--muted)', padding: '1px', display: 'flex' }}>
                    <X size={11} strokeWidth={2} />
                  </button>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* List */}
      <div className="card" style={{ flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column' }}>
        <div className="card__header">
          <div>
            <h2 className="card__title">Entries</h2>
            <p className="card__desc">
              {dictionary.length === 0
                ? 'No entries yet'
                : `${dictionary.length} ${dictionary.length === 1 ? 'entry' : 'entries'}`}
            </p>
          </div>
        </div>
        <div className="card__body" style={{ flex: 1, minHeight: 0, overflowY: 'auto' }}>
          {dictionary.length === 0 ? (
            <div className="empty-state">
              <div className="empty-icon">
                <BookOpen size={16} strokeWidth={1.5} />
              </div>
              <p className="empty-text">No entries yet. Add your first correction above.</p>
            </div>
          ) : (
            <div className="stack-sm">
              {dictionary.map((entry) => (
                <div key={entry.id} className="dict-row">
                  <span className="dict-term">{entry.term}</span>
                  <ArrowRight size={12} strokeWidth={1.75} style={{ flexShrink: 0, color: 'var(--muted)' }} />
                  <span className="dict-replacement">{entry.replacement}</span>
                  <button
                    type="button"
                    onClick={() => deleteDictionaryEntry(entry.id)}
                    aria-label={`Delete ${entry.term}`}
                    className="dict-delete"
                  >
                    <Trash2 size={12} strokeWidth={1.75} />
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
