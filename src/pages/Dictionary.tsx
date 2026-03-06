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
    <div className="flex flex-col h-full overflow-hidden">
      <div className="page-header">
        <h1 className="page-title">Dictionary</h1>
        <p className="page-subtitle">
          Map spoken words or typos to their correct form. Applied automatically during transcription.
        </p>
      </div>

      {error && (
        <Alert variant="destructive" className="mb-4 flex-shrink-0">
          <AlertDescription className="flex items-center justify-between">
            {error}
            <button type="button" onClick={() => setError(null)} aria-label="Dismiss" className="opacity-70 hover:opacity-100 text-base leading-none ml-2">×</button>
          </AlertDescription>
        </Alert>
      )}

      {success && (
        <Alert className="mb-4 flex-shrink-0 border-green-500/30 bg-green-500/10 text-green-400">
          <AlertDescription>Entry saved!</AlertDescription>
        </Alert>
      )}

      {/* Add form */}
      <div className="card mb-4 flex-shrink-0">
        <div className="card__header">
          <div>
            <h2 className="card__title">Add / Update Entry</h2>
            <p className="card__desc">Exact and fuzzy matches are both supported.</p>
          </div>
        </div>
        <div className="card__body">
          <div className="flex items-end gap-2.5">
            <div className="flex flex-col gap-1.5 flex-1 min-w-0">
              <span className="field-label">Spoken / misspelled</span>
              <Input
                value={term}
                onChange={(e) => setTerm(e.target.value)}
                placeholder="e.g. teh, gonna"
                disabled={saving}
              />
            </div>
            <span className="text-[var(--muted-color)] text-base pb-2 flex-shrink-0">→</span>
            <div className="flex flex-col gap-1.5 flex-1 min-w-0">
              <span className="field-label">Replace with</span>
              <Input
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
              className="flex-shrink-0 h-9 px-5"
            >
              {saving ? 'Saving…' : 'Add'}
            </Button>
          </div>
          <p className="text-[11px] text-[var(--muted-color)] mt-2">
            Fuzzy matching corrects near-misses within 2 characters of edit distance.
          </p>
        </div>
      </div>

      {/* Entry list */}
      <div className="card flex-1 min-h-0 flex flex-col">
        <div className="card__header">
          <div>
            <h2 className="card__title">Entries</h2>
            <p className="card__desc">{dictionary.length} {dictionary.length === 1 ? 'entry' : 'entries'}</p>
          </div>
        </div>
        <div className="card__body overflow-y-auto flex-1 min-h-0">
          {dictionary.length === 0 ? (
            <p className="text-[12px] text-[var(--muted-color)]">No entries yet. Add one above.</p>
          ) : (
            <div className="flex flex-col gap-1.5">
              {dictionary.map((entry) => (
                <div key={entry.id} className="flex items-center gap-2.5 px-3 py-2 rounded-lg bg-[var(--surface)] border border-transparent hover:border-[var(--border-subtle)] transition-colors group">
                  <Badge variant="secondary" className="font-mono text-[11px] shrink-0">{entry.term}</Badge>
                  <span className="text-[var(--muted-color)] text-xs">→</span>
                  <span className="flex-1 text-[13px] text-[var(--fg)] truncate">{entry.replacement}</span>
                  <button
                    type="button"
                    onClick={() => deleteDictionaryEntry(entry.id)}
                    aria-label={`Delete ${entry.term}`}
                    className="opacity-0 group-hover:opacity-100 text-[var(--muted-color)] hover:text-red-500 transition-all text-base leading-none px-1 rounded"
                  >×</button>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
