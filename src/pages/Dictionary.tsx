import { useState } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { toast } from 'sonner'
import { ArrowRight, BookOpen, Check, Pencil, Plus, Trash2, X } from 'lucide-react'
import { useDictionary, useUpdateDictionary, useDeleteDictionaryEntry } from '../lib/queries'
import { Button, IconButton } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { PageHeader } from '../components/page'
import { SectionState } from '../components/SectionState'

function DictionarySkeleton() {
  return (
    <div className="flex flex-col">
      {[0, 1, 2, 3].map(i => (
        <div key={i} className="flex items-center gap-6 border-t border-border-soft px-4 py-4 first:border-t-0">
          <div className="nv-skel h-3.5 w-[26%]" />
          <div className="nv-skel h-3.5 w-[26%]" />
          <div className="nv-skel ml-auto h-3.5 w-14" />
        </div>
      ))}
    </div>
  )
}

export function Dictionary() {
  const { data: dictionary = [], status, error, refetch } = useDictionary()
  const updateDictionary = useUpdateDictionary()
  const deleteDictionaryEntry = useDeleteDictionaryEntry()

  const [term, setTerm] = useState('')
  const [replacement, setReplacement] = useState('')
  const [saving, setSaving] = useState(false)

  const [editId, setEditId] = useState<number | null>(null)
  const [editTerm, setEditTerm] = useState('')
  const [editReplacement, setEditReplacement] = useState('')
  const [editSaving, setEditSaving] = useState(false)

  const totalHits = dictionary.reduce((sum, d) => sum + d.hits, 0)
  const maxHits = dictionary.reduce((max, d) => Math.max(max, d.hits), 0)

  const handleAdd = async () => {
    const t = term.trim(), r = replacement.trim()
    if (!t || !r) return
    setSaving(true)
    try {
      await updateDictionary.mutateAsync({ term: t, replacement: r })
      setTerm(''); setReplacement('')
      toast.success('Entry saved')
    } catch {
      // The mutation reports the error; keep the inputs available for retry.
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
      await updateDictionary.mutateAsync({
        term: t,
        replacement: r,
        previousTerm: dictionary.find((d) => d.id === editId)?.term,
      })
      cancelEdit()
    } catch {
      // The mutation reports the error; preserve the edit for retry.
    } finally { setEditSaving(false) }
  }

  return (
    <div className="nv-page">
      <PageHeader
        title="Dictionary"
        description="Words NexusVoice should always write your way. Each replacement is applied to the transcript before it is pasted."
        actions={dictionary.length > 0 && (
          <span className="text-[12.5px] text-muted tabular-nums">
            {dictionary.length} {dictionary.length === 1 ? 'entry' : 'entries'}, used {totalHits.toLocaleString()} {totalHits === 1 ? 'time' : 'times'}
          </span>
        )}
      />

      <section className="nv-card nv-composer" aria-label="Add an entry">
        <div className="nv-composer__fields">
          <label className="min-w-0">
            <span className="nv-field-label">Heard as</span>
            <Input
              value={term}
              onChange={(e) => setTerm(e.target.value)}
              placeholder="e.g. teh, gonna"
              disabled={saving}
            />
          </label>
          <span className="nv-composer__arrow" aria-hidden><ArrowRight /></span>
          <label className="min-w-0">
            <span className="nv-field-label">Replace with</span>
            <Input
              value={replacement}
              onChange={(e) => setReplacement(e.target.value)}
              placeholder="e.g. the, going to"
              disabled={saving}
              onKeyDown={(e) => { if (e.key === 'Enter') handleAdd() }}
            />
          </label>
          <Button
            onClick={handleAdd}
            disabled={saving || !term.trim() || !replacement.trim()}
            className="h-8.5!"
          >
            <Plus />
            {saving ? 'Saving…' : 'Add to dictionary'}
          </Button>
        </div>
      </section>

      <section className="nv-card nv-table-card mt-4" aria-label="Entries">
        <SectionState status={status} error={error?.message} onRetry={refetch} skeleton={<DictionarySkeleton />}>
          {dictionary.length === 0 ? (
            <div className="nv-empty">
              <span className="nv-mark nv-mark--lg nv-mark--accent nv-empty__mark">
                <BookOpen size={20} strokeWidth={1.8} />
              </span>
              <p className="nv-empty__title">No entries yet. Add your first correction above.</p>
              <p className="nv-empty__desc">Teach it names, jargon and the words it keeps getting wrong.</p>
            </div>
          ) : (
            <table className="nv-table">
              <thead>
                <tr>
                  <th>Heard as</th>
                  <th className="is-arrow" aria-hidden />
                  <th>Replaced with</th>
                  <th className="is-num w-32">Used</th>
                  <th className="is-actions"><span className="sr-only">Actions</span></th>
                </tr>
              </thead>
              <tbody>
                <AnimatePresence initial={false}>
                  {dictionary.map((entry) => {
                    const editing = editId === entry.id
                    return (
                      <motion.tr
                        key={entry.id}
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        transition={{ duration: 0.16 }}
                      >
                        <td>
                          {editing
                            ? <Input value={editTerm} onChange={(e) => setEditTerm(e.target.value)} disabled={editSaving} className="nv-input--sm" aria-label="Heard as" />
                            : <span className="nv-term">{entry.term}</span>}
                        </td>
                        <td className="is-arrow"><ArrowRight aria-hidden /></td>
                        <td>
                          {editing
                            ? <Input value={editReplacement} onChange={(e) => setEditReplacement(e.target.value)} disabled={editSaving} onKeyDown={(e) => { if (e.key === 'Enter') commitEdit() }} className="nv-input--sm" aria-label="Replace with" />
                            : <span className="nv-replacement">{entry.replacement}</span>}
                        </td>
                        <td className="is-num">
                          <span className="nv-uses" title={`Replaced ${entry.hits} ${entry.hits === 1 ? 'time' : 'times'}`}>
                            <span className="nv-uses__track" aria-hidden>
                              <span className="nv-uses__fill" style={{ width: `${maxHits ? (entry.hits / maxHits) * 100 : 0}%` }} />
                            </span>
                            {entry.hits}
                          </span>
                        </td>
                        <td className="is-actions">
                          <span className="nv-row-actions" data-active={editing || undefined}>
                            {editing ? (
                              <>
                                <IconButton label="Save entry" tone="success" onClick={commitEdit} disabled={editSaving}>
                                  <Check strokeWidth={2.4} />
                                </IconButton>
                                <IconButton label="Cancel editing" tone="danger" onClick={cancelEdit}>
                                  <X strokeWidth={2} />
                                </IconButton>
                              </>
                            ) : (
                              <>
                                <IconButton label={`Edit ${entry.term}`} tone="accent" onClick={() => startEdit(entry.id, entry.term, entry.replacement)}>
                                  <Pencil strokeWidth={1.9} />
                                </IconButton>
                                <IconButton label={`Delete ${entry.term}`} tone="danger" onClick={() => deleteDictionaryEntry.mutate(entry.id)}>
                                  <Trash2 strokeWidth={1.9} />
                                </IconButton>
                              </>
                            )}
                          </span>
                        </td>
                      </motion.tr>
                    )
                  })}
                </AnimatePresence>
              </tbody>
            </table>
          )}
        </SectionState>
      </section>
    </div>
  )
}
