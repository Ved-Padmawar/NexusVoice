import { useMemo, useState } from 'react'
import { toast } from 'sonner'
import { Trash2, Pencil, Check, X, Search, Plus, BookOpen } from 'lucide-react'
import { useDictionary, useUpdateDictionary, useDeleteDictionaryEntry } from '../lib/queries'
import { Input } from '@/components/ui/input'
import { PageBar } from '../components/PageBar'
import { SectionState } from '../components/SectionState'

/** Word · heard as · fixed · actions. One grid for the header and every row. */
const ROW = 'grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_72px_60px] items-center gap-4'

function DictionarySkeleton() {
  return (
    <div className="flex flex-col">
      {[0, 1, 2, 3, 4, 5].map(i => (
        <div key={i} className={`${ROW} border-b border-(--hairline) px-4 py-2.5 last:border-0`}>
          <div className="h-3.5 w-32 animate-pulse rounded bg-(--surface)" />
          <div className="h-3 w-24 animate-pulse rounded bg-(--surface)" />
          <div className="h-3 w-8 animate-pulse justify-self-center rounded bg-(--surface)" />
          <div />
        </div>
      ))}
    </div>
  )
}

/**
 * The correction engine matches by Levenshtein distance and Double Metaphone,
 * so an entry catches near-misses and sound-alikes too, not just the exact
 * string. The row therefore leads with the word being taught and treats the
 * mis-hearing as the example that seeded it.
 */
export function Dictionary() {
  const { data: dictionary = [], status, error, refetch } = useDictionary()
  const updateDictionary = useUpdateDictionary()
  const deleteDictionaryEntry = useDeleteDictionaryEntry()

  const [term, setTerm] = useState('')
  const [replacement, setReplacement] = useState('')
  const [saving, setSaving] = useState(false)
  const [query, setQuery] = useState('')

  const [editId, setEditId] = useState<number | null>(null)
  const [editTerm, setEditTerm] = useState('')
  const [editReplacement, setEditReplacement] = useState('')
  const [editSaving, setEditSaving] = useState(false)

  const canAdd = term.trim() !== '' && replacement.trim() !== ''

  // Most-corrected first: entries earning their place lead, dead ones collect
  // at the bottom where they are easy to prune.
  const visible = useMemo(() => {
    const q = query.trim().toLowerCase()
    const rows = q
      ? dictionary.filter(e =>
          e.term.toLowerCase().includes(q) || e.replacement.toLowerCase().includes(q))
      : dictionary
    return [...rows].sort((a, b) => b.hits - a.hits || a.replacement.localeCompare(b.replacement))
  }, [dictionary, query])

  const handleAdd = async () => {
    const t = term.trim(), r = replacement.trim()
    if (!t || !r) return
    setSaving(true)
    try {
      await updateDictionary.mutateAsync({ term: t, replacement: r })
      setTerm(''); setReplacement('')
      toast.success('Correction added')
    } catch {
      // The mutation reports the error; keep the inputs available for retry.
    } finally { setSaving(false) }
  }

  const startEdit = (id: number, t: string, r: string) => {
    setEditId(id); setEditTerm(t); setEditReplacement(r)
  }
  const cancelEdit = () => { setEditId(null); setEditTerm(''); setEditReplacement('') }

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
    <div className="flex h-full flex-col overflow-hidden">
      <PageBar
        title="Dictionary"
        description="Words you're teaching NexusVoice to hear correctly"
      />

      <div className="mx-auto mb-(--dock-clear) flex min-h-0 w-full max-w-(--measure) flex-1 flex-col gap-5 overflow-hidden px-(--gutter) pt-1">

        {/* One container. The composer is the first band, the column header
            the second, and the rows fill the rest — so adding a word and the
            words themselves read as one object rather than two slabs. */}
        <div className="panel flex min-h-0 flex-1 flex-col overflow-hidden">

          <div className="flex shrink-0 items-center gap-2.5 px-4 py-3">
            <span className="shrink-0 text-[12.5px] text-(--muted)">When it hears</span>
            <Input
              value={term}
              onChange={(e) => setTerm(e.target.value)}
              placeholder="neksus"
              aria-label="Word as heard"
              disabled={saving}
              className="h-8 min-w-0 flex-1 text-[12.5px]"
              onKeyDown={(e) => { if (e.key === 'Enter') handleAdd() }}
            />
            <span className="shrink-0 text-[12.5px] text-(--muted)">write</span>
            <Input
              value={replacement}
              onChange={(e) => setReplacement(e.target.value)}
              placeholder="NexusVoice"
              aria-label="Correct it to"
              disabled={saving}
              className="h-8 min-w-0 flex-1 text-[12.5px]"
              onKeyDown={(e) => { if (e.key === 'Enter') handleAdd() }}
            />
            <button
              type="button"
              onClick={handleAdd}
              disabled={saving || !canAdd}
              className="btn btn-sm btn-primary shrink-0"
            >
              <Plus size={12} strokeWidth={2.5} />
              {saving ? 'Adding…' : 'Add word'}
            </button>

            {dictionary.length > 0 && (
              <>
                <span className="rule mx-1 h-6 w-px shrink-0" aria-hidden />
                <div className="relative flex shrink-0 items-center">
                  <Search size={12} strokeWidth={2} className="pointer-events-none absolute left-2.5 text-(--faint)" />
                  <Input
                    value={query}
                    onChange={e => setQuery(e.target.value)}
                    placeholder="Filter words…"
                    aria-label="Filter words"
                    className="h-8 w-44 pl-7 text-[12.5px]"
                  />
                </div>
              </>
            )}
          </div>

          <div className="rule h-px" />

          <div className={`${ROW} shrink-0 border-b border-(--hairline) px-4 py-2`}>
            <span className="flex items-center gap-2 text-[10.5px] font-medium text-(--muted)">
              Word
              {dictionary.length > 0 && (
                <span className="tabular-nums text-(--faint)">{visible.length}</span>
              )}
            </span>
            <span className="text-[10.5px] font-medium text-(--muted)">Heard as</span>
            <span className="justify-self-center text-[10.5px] font-medium text-(--muted)">Fixed</span>
            <span />
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto overscroll-none">
              <SectionState status={status} error={error?.message} onRetry={refetch} skeleton={<DictionarySkeleton />}>
                {dictionary.length === 0 ? (
                  <div className="flex flex-col items-center gap-2.5 px-6 py-16 text-center">
                    <span className="grid size-10 place-items-center rounded-full bg-(--surface) text-(--faint)">
                      <BookOpen size={17} strokeWidth={1.6} />
                    </span>
                    <p className="m-0 text-[13px] font-semibold text-(--fg-2)">No words yet</p>
                    <p className="m-0 max-w-76 text-[12px] leading-[1.6] text-(--muted)">
                      Add a name, acronym or piece of jargon above. Close matches
                      and sound-alikes are corrected too, so one entry usually
                      covers every way it gets misheard.
                    </p>
                  </div>
                ) : visible.length === 0 ? (
                  <div className="flex flex-col items-center gap-1.5 px-6 py-16 text-center">
                    <p className="m-0 text-[13px] font-semibold text-(--fg-2)">No matches</p>
                    <p className="m-0 text-[12px] text-(--muted)">Nothing matches “{query.trim()}”.</p>
                  </div>
                ) : (
                  visible.map((entry) => {
                    const editing = editId === entry.id
                    return (
                      <div
                        key={entry.id}
                        className={`${ROW} group border-b border-(--hairline) px-4 py-2 transition-colors duration-(--t-fast) last:border-0 ${
                          editing ? 'bg-(--surface)' : 'hover:bg-(--surface)'
                        }`}
                      >
                        {editing ? (
                          <>
                            <Input
                              value={editReplacement}
                              onChange={(e) => setEditReplacement(e.target.value)}
                              disabled={editSaving}
                              aria-label="Replacement"
                              className="h-7 text-[12.5px]"
                              onKeyDown={(e) => { if (e.key === 'Enter') commitEdit() }}
                            />
                            <Input
                              value={editTerm}
                              onChange={(e) => setEditTerm(e.target.value)}
                              disabled={editSaving}
                              aria-label="Heard as"
                              className="h-7 text-[12.5px]"
                              onKeyDown={(e) => { if (e.key === 'Enter') commitEdit() }}
                            />
                            <span />
                            <div className="flex justify-self-end">
                              <button
                                type="button" onClick={commitEdit} disabled={editSaving}
                                title="Save changes" aria-label="Save changes"
                                className="iconbtn hover:bg-(--success-soft) hover:text-(--success)"
                              >
                                <Check size={13} strokeWidth={2.5} />
                              </button>
                              <button
                                type="button" onClick={cancelEdit}
                                title="Cancel" aria-label="Cancel edit"
                                className="iconbtn iconbtn-danger"
                              >
                                <X size={13} strokeWidth={2} />
                              </button>
                            </div>
                          </>
                        ) : (
                          <>
                            <span data-selectable className="truncate text-[12.5px] font-medium text-(--fg)">
                              {entry.replacement}
                            </span>
                            <span data-selectable className="truncate text-[12.5px] text-(--muted)">
                              {entry.term}
                            </span>
                            <span
                              className={`justify-self-center text-[11.5px] tabular-nums ${
                                entry.hits > 0 ? 'text-(--on-soft)' : 'text-(--faint)'
                              }`}
                            >
                              {entry.hits > 0 ? `${entry.hits}×` : '—'}
                            </span>
                            <div className="row-actions flex justify-self-end">
                              <button
                                type="button"
                                onClick={() => startEdit(entry.id, entry.term, entry.replacement)}
                                title="Edit" aria-label={`Edit ${entry.term}`}
                                className="iconbtn iconbtn-accent size-6"
                              >
                                <Pencil size={12} strokeWidth={1.9} />
                              </button>
                              <button
                                type="button"
                                onClick={() => deleteDictionaryEntry.mutate(entry.id)}
                                title="Delete" aria-label={`Delete ${entry.term}`}
                                className="iconbtn iconbtn-danger size-6"
                              >
                                <Trash2 size={12} strokeWidth={1.9} />
                              </button>
                            </div>
                          </>
                        )}
                      </div>
                    )
                  })
                )}
            </SectionState>
          </div>
        </div>
      </div>
    </div>
  )
}
