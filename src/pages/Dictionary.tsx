import { useState } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { toast } from 'sonner'
import { Trash2, BookOpen, Plus, Pencil, Check, X, Mic } from 'lucide-react'
import { useDictionary, useUpdateDictionary, useDeleteDictionaryEntry } from '../lib/queries'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { SectionState } from '../components/SectionState'

function DictionarySkeleton() {
  return (
    <div className="flex flex-col">
      {[0, 1, 2, 3, 4].map(i => (
        <div key={i} className="flex items-center gap-4 px-4 py-3.5 border-b border-(--border-soft) last:border-none">
          <div className="h-3.5 w-[28%] rounded bg-(--surface) animate-pulse" />
          <div className="h-3.5 w-[28%] rounded bg-(--surface) animate-pulse" />
          <div className="h-3.5 w-8 rounded bg-(--surface) animate-pulse ml-auto" />
          <div className="h-3.5 w-12 rounded bg-(--surface) animate-pulse" />
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
    <div className="flex flex-col h-full overflow-hidden">
      <div className="flex-1 min-h-0 overflow-hidden px-8 pt-7 pb-4 flex flex-col gap-5">

        {/* Hero */}
        <div className="flex items-center justify-between gap-4 pb-3.5 border-b border-(--border-soft)">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-(--r-lg) bg-(--accent-soft) text-(--accent) flex items-center justify-center shrink-0">
              <BookOpen size={16} strokeWidth={2} />
            </div>
            <div className="min-w-0">
              <h1 className="text-[16px] font-bold tracking-tight text-(--fg) leading-[1.1] m-0">Dictionary</h1>
              <p className="text-[11px] text-muted-foreground mt-0.5 m-0 truncate">Custom phonetics and word replacements.</p>
            </div>
          </div>
        </div>

        {/* Quick Addition */}
        <div className="bg-(--panel) border border-(--border) rounded-(--r-xl) px-4.5 py-3 shrink-0">
          <div className="flex items-center gap-1.75 mb-2.5">
            <Plus size={12} strokeWidth={2} className="text-(--accent)" />
            <span className="text-[12px] font-semibold text-(--fg-2) tracking-[-0.01em]">Quick addition</span>
          </div>
          <div className="grid grid-cols-[1fr_1fr_auto] gap-2.5 items-end">
            <div className="flex flex-col gap-1.5">
              <label className="text-[11px] font-medium text-muted-foreground">Trigger word</label>
              <Input
                value={term}
                onChange={(e) => setTerm(e.target.value)}
                placeholder="e.g. teh, gonna"
                disabled={saving}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <label className="text-[11px] font-medium text-muted-foreground">Corrected text</label>
              <Input
                value={replacement}
                onChange={(e) => setReplacement(e.target.value)}
                placeholder="e.g. the, going to"
                disabled={saving}
                onKeyDown={(e) => { if (e.key === 'Enter') handleAdd() }}
              />
            </div>
            <div className="flex items-end">
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

        {/* Vocabulary Table */}
        <div className="flex flex-col flex-1 min-h-0 gap-2.5 overflow-hidden">
          <div className="flex items-center justify-between shrink-0">
            <h2 className="text-[13px] font-semibold tracking-[-0.01em] text-(--fg-2) m-0">Vocabulary engine</h2>
          </div>

          <div className="flex-1 min-h-0 border border-(--border) rounded-(--r-xl) bg-background overflow-hidden flex flex-col">
            <div className="flex-1 min-h-0 overflow-y-auto">
              <SectionState status={status} error={error?.message} onRetry={refetch} skeleton={<DictionarySkeleton />}>
              {dictionary.length === 0 ? (
                <div className="flex flex-col items-center gap-2 py-8 px-4 text-center">
                  <div className="w-9 h-9 rounded-(--r-lg) bg-(--surface) border border-(--border-soft) flex items-center justify-center text-muted-foreground opacity-80">
                    <BookOpen size={16} strokeWidth={1.5} />
                  </div>
                  <p className="text-[12px] text-muted-foreground max-w-60 leading-normal m-0">No entries yet. Add your first correction above.</p>
                </div>
              ) : (
                <table className="w-full border-collapse text-left">
                  <thead>
                    <tr className="bg-(--panel) border-b border-(--border)">
                      <th className="px-4 py-2.75 text-[11px] font-medium text-muted-foreground whitespace-nowrap w-[30%]">Input trigger</th>
                      <th className="px-4 py-2.75 text-[11px] font-medium text-muted-foreground whitespace-nowrap w-[30%]">Output correction</th>
                      <th className="px-4 py-2.75 text-[11px] font-medium text-muted-foreground whitespace-nowrap w-25 text-center">Hits</th>
                      <th className="px-4 py-2.75 text-[11px] font-medium text-muted-foreground whitespace-nowrap w-30 text-right">Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    <AnimatePresence initial={false}>
                      {dictionary.map((entry) => (
                        <motion.tr
                          key={entry.id}
                          className="border-b border-(--border-soft) last:border-none transition-colors duration-(--t-fast) hover:bg-(--surface)"
                          initial={{ opacity: 0 }}
                          animate={{ opacity: 1 }}
                          exit={{ opacity: 0, height: 0 }}
                          transition={{ duration: 0.15 }}
                          layout
                        >
                          {editId === entry.id ? (
                            <>
                              <td className="px-4 py-3.5 text-[13px] text-(--fg)">
                                <Input value={editTerm} onChange={(e) => setEditTerm(e.target.value)} disabled={editSaving} className="h-7.5! text-[12px]! px-2!" />
                              </td>
                              <td className="px-4 py-3.5 text-[13px] text-(--fg)">
                                <Input value={editReplacement} onChange={(e) => setEditReplacement(e.target.value)} disabled={editSaving} onKeyDown={(e) => { if (e.key === 'Enter') commitEdit() }} className="h-7.5! text-[12px]! px-2!" />
                              </td>
                              <td className="px-4 py-3.5 text-center">
                                <span className="text-[11px] font-bold text-muted-foreground">{entry.hits}</span>
                              </td>
                              <td className="px-4 py-3.5 text-right">
                                <div className="flex items-center justify-end gap-0.5">
                                  <button type="button" className="w-7 h-7 flex items-center justify-center rounded-(--r-md) border-none bg-transparent cursor-pointer text-muted-foreground transition-[background,color] duration-(--t-fast) hover:bg-[color-mix(in_srgb,var(--success)_12%,transparent)] hover:text-(--success)" onClick={commitEdit} disabled={editSaving}>
                                    <Check size={14} strokeWidth={2.5} />
                                  </button>
                                  <button type="button" className="w-7 h-7 flex items-center justify-center rounded-(--r-md) border-none bg-transparent cursor-pointer text-muted-foreground transition-[background,color] duration-(--t-fast) hover:bg-[color-mix(in_srgb,var(--danger)_12%,transparent)] hover:text-destructive" onClick={cancelEdit}>
                                    <X size={14} strokeWidth={2} />
                                  </button>
                                </div>
                              </td>
                            </>
                          ) : (
                            <>
                              <td className="px-4 py-3.5 text-[13px] text-(--fg)">
                                <div className="flex items-center gap-2.5">
                                  <Mic size={13} strokeWidth={1.75} className="text-muted-foreground shrink-0" />
                                  <span className="text-[13px] font-medium text-(--fg)">{entry.term}</span>
                                </div>
                              </td>
                              <td className="px-4 py-3.5 text-[13px] text-(--fg)">
                                <span className="font-mono text-[12px] text-(--accent) bg-(--accent-soft) px-2 py-0.5 rounded-(--r-sm)" style={{ border: '1px solid color-mix(in srgb, var(--accent) 20%, transparent)' }}>{entry.replacement}</span>
                              </td>
                              <td className="px-4 py-3.5 text-center">
                                <span className="text-[11px] font-bold text-muted-foreground">{entry.hits}</span>
                              </td>
                              <td className="px-4 py-3.5 text-right">
                                <div className="flex items-center justify-end gap-0.5">
                                  <button type="button" className="w-7 h-7 flex items-center justify-center rounded-(--r-md) border-none bg-transparent cursor-pointer text-muted-foreground transition-[background,color] duration-(--t-fast) hover:bg-(--accent-soft) hover:text-(--accent)" onClick={() => startEdit(entry.id, entry.term, entry.replacement)}>
                                    <Pencil size={14} strokeWidth={1.75} />
                                  </button>
                                  <button type="button" className="w-7 h-7 flex items-center justify-center rounded-(--r-md) border-none bg-transparent cursor-pointer text-muted-foreground transition-[background,color] duration-(--t-fast) hover:bg-[color-mix(in_srgb,var(--danger)_12%,transparent)] hover:text-destructive" onClick={() => deleteDictionaryEntry.mutate(entry.id)}>
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
              </SectionState>
            </div>
          </div>
        </div>

      </div>
    </div>
  )
}
