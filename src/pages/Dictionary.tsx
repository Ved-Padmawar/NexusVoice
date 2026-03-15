import { useState } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { ArrowRight, Trash2, BookOpen, CheckCircle2, AlertCircle, X } from 'lucide-react'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

export function Dictionary() {
  const { dictionary, updateDictionary, deleteDictionaryEntry, error, setError } = useAppStore()
  const [term, setTerm] = useState('')
  const [replacement, setReplacement] = useState('')
  const [saving, setSaving] = useState(false)
  const [success, setSuccess] = useState(false)

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
    <div className="dict-page">
      <div className="page-header">
        <h1 className="page-title">Dictionary</h1>
        <p className="page-subtitle">
          Words you use 3+ times are learned automatically. Add manual corrections below.
        </p>
      </div>

      <AnimatePresence>
        {error && (
          <motion.div key="dict-error" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
            <div className="notice notice--error">
              <AlertCircle size={13} strokeWidth={2} className="icon--shrink icon--danger" />
              <span className="text--flex">{error}</span>
              <button type="button" className="notice__close" onClick={() => setError(null)}>
                <X size={13} strokeWidth={2} />
              </button>
            </div>
          </motion.div>
        )}
        {success && (
          <motion.div key="dict-success" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
            <div className="notice notice--success">
              <CheckCircle2 size={13} strokeWidth={2} className="icon--shrink icon--success" />
              <span>Entry saved successfully.</span>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Add form */}
      <div className="card">
        <div className="card__header">
          <div>
            <h2 className="card__title">Add / Update Entry</h2>
            <p className="card__desc">Fuzzy matching corrects near-misses within 2 characters.</p>
          </div>
        </div>
        <div className="card__body">
          <div className="dict-add-form">
            <div className="dict-add-field">
              <Label htmlFor="dict-term">Spoken / misspelled</Label>
              <Input
                id="dict-term"
                value={term}
                onChange={(e) => setTerm(e.target.value)}
                placeholder="e.g. teh, gonna"
                disabled={saving}
              />
            </div>
            <div className="dict-add-arrow">
              <ArrowRight size={14} strokeWidth={1.75} />
            </div>
            <div className="dict-add-field">
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
            <Button type="button" onClick={handleAdd} disabled={saving || !term.trim() || !replacement.trim()}>
              {saving ? 'Saving…' : 'Add'}
            </Button>
          </div>
        </div>
      </div>

      {/* List */}
      <div className="card dict-list-body">
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
        <div className="card__body dict-list-scroll">
          {dictionary.length === 0 ? (
            <div className="empty-state">
              <div className="empty-icon"><BookOpen size={16} strokeWidth={1.5} /></div>
              <p className="empty-text">No entries yet. Add your first correction above.</p>
            </div>
          ) : (
            <div className="stack-sm">
              <AnimatePresence initial={false}>
                {dictionary.map((entry) => (
                  <motion.div
                    key={entry.id}
                    className="dict-row"
                    initial={{ opacity: 0, x: -8 }}
                    animate={{ opacity: 1, x: 0 }}
                    exit={{ opacity: 0, height: 0, marginBottom: 0 }}
                    transition={{ duration: 0.18 }}
                    layout
                  >
                    <span className="dict-term">{entry.term}</span>
                    <ArrowRight size={12} strokeWidth={1.75} className="icon--shrink icon--muted" />
                    <span className="dict-replacement">{entry.replacement}</span>
                    <button
                      type="button"
                      onClick={() => deleteDictionaryEntry(entry.id)}
                      aria-label={`Delete ${entry.term}`}
                      className="dict-delete"
                    >
                      <Trash2 size={12} strokeWidth={1.75} />
                    </button>
                  </motion.div>
                ))}
              </AnimatePresence>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
