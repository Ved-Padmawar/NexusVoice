import { useState } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { Trash2, BookOpen, CheckCircle2, AlertCircle, X, Plus, Pencil, Check, Mic } from 'lucide-react'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

export function Dictionary() {
  const { dictionary, updateDictionary, deleteDictionaryEntry, error, setError } = useAppStore()

  // Add form
  const [term, setTerm] = useState('')
  const [replacement, setReplacement] = useState('')
  const [saving, setSaving] = useState(false)
  const [success, setSuccess] = useState(false)

  // Inline edit
  const [editId, setEditId] = useState<number | null>(null)
  const [editTerm, setEditTerm] = useState('')
  const [editReplacement, setEditReplacement] = useState('')
  const [editSaving, setEditSaving] = useState(false)

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

  const startEdit = (id: number, t: string, r: string) => {
    setEditId(id); setEditTerm(t); setEditReplacement(r)
  }

  const cancelEdit = () => {
    setEditId(null); setEditTerm(''); setEditReplacement('')
  }

  const commitEdit = async () => {
    const t = editTerm.trim(), r = editReplacement.trim()
    if (!t || !r) return
    setEditSaving(true)
    try {
      await updateDictionary(t, r)
      cancelEdit()
    } finally { setEditSaving(false) }
  }

  return (
    <div className="dict-page">

      <div className="dict-scroll">

      {/* ── Hero ─────────────────────────────────────────────────── */}
      <div className="db__hero">
        <div className="db__hero-glow" aria-hidden />
        <div className="db__hero-body">
          <div className="db__hero-icon">
            <BookOpen size={18} strokeWidth={2} />
          </div>
          <div>
            <h1 className="db__hero-title">Dictionary</h1>
            <p className="db__hero-sub">Custom phonetics and word replacements.</p>
          </div>
        </div>
      </div>

      {/* ── Notices ──────────────────────────────────────────────── */}
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

      {/* ── Quick Addition ───────────────────────────────────────── */}
      <div className="dict-add-card">
        <div className="dict-add-card__header">
          <Plus size={14} strokeWidth={2} className="icon--accent" />
          <span className="dict-add-card__label">Quick Addition</span>
        </div>
        <div className="dict-add-card__form">
          <div className="dict-add-card__field">
            <label className="dict-add-card__field-label">Trigger Word</label>
            <Input
              value={term}
              onChange={(e) => setTerm(e.target.value)}
              placeholder="e.g. teh, gonna"
              disabled={saving}
            />
          </div>
          <div className="dict-add-card__field">
            <label className="dict-add-card__field-label">Corrected Text</label>
            <Input
              value={replacement}
              onChange={(e) => setReplacement(e.target.value)}
              placeholder="e.g. the, going to"
              disabled={saving}
              onKeyDown={(e) => { if (e.key === 'Enter') handleAdd() }}
            />
          </div>
          <div className="dict-add-card__actions">
            <Button
              type="button"
              onClick={handleAdd}
              disabled={saving || !term.trim() || !replacement.trim()}
            >
              {saving ? 'Saving…' : 'Add to Dictionary'}
            </Button>
          </div>
        </div>
      </div>

      {/* ── Vocabulary Table ─────────────────────────────────────── */}
      <div className="dict-table-section">
        <div className="dict-table-section__header">
          <h2 className="dict-table-section__title">Vocabulary Engine</h2>
        </div>

        <div className="dict-table-wrap">
          <div className="dict-table-scroll">
          {dictionary.length === 0 ? (
            <div className="empty-state">
              <div className="empty-icon"><BookOpen size={16} strokeWidth={1.5} /></div>
              <p className="empty-text">No entries yet. Add your first correction above.</p>
            </div>
          ) : (
            <table className="dict-table">
              <thead>
                <tr className="dict-table__head-row">
                  <th className="dict-table__th dict-table__th--trigger">Input Trigger</th>
                  <th className="dict-table__th dict-table__th--correction">Output Correction</th>
                  <th className="dict-table__th dict-table__th--num">Hits</th>
                  <th className="dict-table__th dict-table__th--right">Actions</th>
                </tr>
              </thead>
              <tbody>
                <AnimatePresence initial={false}>
                  {dictionary.map((entry) => (
                    <motion.tr
                      key={entry.id}
                      className="dict-table__row"
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      exit={{ opacity: 0, height: 0 }}
                      transition={{ duration: 0.15 }}
                      layout
                    >
                      {editId === entry.id ? (
                        <>
                          <td className="dict-table__td">
                            <Input
                              value={editTerm}
                              onChange={(e) => setEditTerm(e.target.value)}
                              disabled={editSaving}
                              className="dict-table__edit-input"
                            />
                          </td>
                          <td className="dict-table__td">
                            <Input
                              value={editReplacement}
                              onChange={(e) => setEditReplacement(e.target.value)}
                              disabled={editSaving}
                              onKeyDown={(e) => { if (e.key === 'Enter') commitEdit() }}
                              className="dict-table__edit-input"
                            />
                          </td>
                          <td className="dict-table__td dict-table__td--num">
                            <span className="dict-table__hits">{entry.hits}</span>
                          </td>
                          <td className="dict-table__td dict-table__td--right">
                            <div className="dict-table__actions">
                              <button
                                type="button"
                                className="dict-table__btn dict-table__btn--confirm"
                                onClick={commitEdit}
                                disabled={editSaving}
                              >
                                <Check size={14} strokeWidth={2.5} />
                              </button>
                              <button
                                type="button"
                                className="dict-table__btn"
                                onClick={cancelEdit}
                              >
                                <X size={14} strokeWidth={2} />
                              </button>
                            </div>
                          </td>
                        </>
                      ) : (
                        <>
                          <td className="dict-table__td">
                            <div className="dict-table__trigger">
                              <Mic size={13} strokeWidth={1.75} className="dict-table__trigger-icon" />
                              <span className="dict-table__trigger-text">{entry.term}</span>
                            </div>
                          </td>
                          <td className="dict-table__td">
                            <span className="dict-table__replacement">{entry.replacement}</span>
                          </td>
                          <td className="dict-table__td dict-table__td--num">
                            <span className="dict-table__hits">{entry.hits}</span>
                          </td>
                          <td className="dict-table__td dict-table__td--right">
                            <div className="dict-table__actions">
                              <button
                                type="button"
                                className="dict-table__btn"
                                onClick={() => startEdit(entry.id, entry.term, entry.replacement)}
                              >
                                <Pencil size={14} strokeWidth={1.75} />
                              </button>
                              <button
                                type="button"
                                className="dict-table__btn dict-table__btn--danger"
                                onClick={() => deleteDictionaryEntry(entry.id)}
                              >
                                <Trash2 size={14} strokeWidth={1.75} />
                              </button>
                            </div>
                          </td>
                        </>
                      )}
                    </motion.tr>
                  ))}
                </AnimatePresence>
              </tbody>
            </table>
          )}
          </div>{/* dict-table-scroll */}
        </div>
      </div>

      </div>{/* dict-scroll */}
    </div>
  )
}
