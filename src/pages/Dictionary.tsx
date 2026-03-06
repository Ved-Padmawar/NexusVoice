import { useState } from 'react'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Alert, AlertDescription } from '@/components/ui/alert'

export function Dictionary() {
  const { dictionary, updateDictionary, deleteDictionaryEntry, error, setError } = useAppStore()

  const [term, setTerm] = useState('')
  const [replacement, setReplacement] = useState('')
  const [saving, setSaving] = useState(false)
  const [success, setSuccess] = useState(false)

  const handleAdd = async () => {
    const t = term.trim()
    const r = replacement.trim()
    if (!t || !r) return
    setSaving(true)
    setError(null)
    setSuccess(false)
    try {
      await updateDictionary(t, r)
      setTerm('')
      setReplacement('')
      setSuccess(true)
      setTimeout(() => setSuccess(false), 2500)
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="dictionary-page">
      <div className="page-header">
        <h1 className="page-title">Dictionary</h1>
        <p className="page-subtitle">
          Map spoken words or typos to their correct form. Applied automatically during transcription.
        </p>
      </div>

      {error && (
        <Alert variant="destructive" className="mb-4">
          <AlertDescription className="flex items-center justify-between">
            {error}
            <button type="button" onClick={() => setError(null)} aria-label="Dismiss" className="opacity-70 hover:opacity-100 text-base leading-none ml-2">×</button>
          </AlertDescription>
        </Alert>
      )}

      {success && (
        <Alert className="mb-4 border-green-500/30 bg-green-500/10 text-green-400">
          <AlertDescription>Entry saved!</AlertDescription>
        </Alert>
      )}

      {/* Add form */}
      <div className="card settings-section" style={{ marginBottom: 16 }}>
        <div className="card__header">
          <div>
            <h2 className="card__title">Add / Update Entry</h2>
            <p className="card__desc">Exact and fuzzy matches are both supported.</p>
          </div>
        </div>
        <div className="card__body">
          <div style={{ display: 'flex', gap: 8, alignItems: 'flex-end' }}>
            <div style={{ flex: 1 }}>
              <span className="field-label">Spoken / misspelled</span>
              <Input
                value={term}
                onChange={(e) => setTerm(e.target.value)}
                placeholder="e.g. teh, gonna, API"
                disabled={saving}
                style={{ marginTop: 4 }}
              />
            </div>
            <div style={{ flex: 1 }}>
              <span className="field-label">Replace with</span>
              <Input
                value={replacement}
                onChange={(e) => setReplacement(e.target.value)}
                placeholder="e.g. the, going to, API"
                disabled={saving}
                style={{ marginTop: 4 }}
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
          <p className="field-hint" style={{ marginTop: 6 }}>
            Fuzzy matching corrects near-misses within 2 characters of edit distance.
          </p>
        </div>
      </div>

      {/* Entry list */}
      <div className="card settings-section">
        <div className="card__header">
          <div>
            <h2 className="card__title">Entries</h2>
            <p className="card__desc">{dictionary.length} {dictionary.length === 1 ? 'entry' : 'entries'}</p>
          </div>
        </div>
        <div className="card__body">
          {dictionary.length === 0 ? (
            <p className="field-hint">No entries yet. Add one above.</p>
          ) : (
            <div className="dict-list">
              {dictionary.map((entry) => (
                <div key={entry.id} className="dict-row">
                  <Badge variant="secondary" className="dict-term">{entry.term}</Badge>
                  <span className="dict-arrow">→</span>
                  <span className="dict-replacement">{entry.replacement}</span>
                  <button
                    type="button"
                    className="dict-delete"
                    onClick={() => deleteDictionaryEntry(entry.id)}
                    aria-label={`Delete ${entry.term}`}
                    title="Delete entry"
                  >
                    ×
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
