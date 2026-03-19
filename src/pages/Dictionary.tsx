import { useState } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { toast } from 'sonner'
import { Trash2, BookOpen, AlertCircle, X, Plus, Pencil, Check, Mic } from 'lucide-react'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

export function Dictionary() {
  const { dictionary, updateDictionary, deleteDictionaryEntry, error, setError } = useAppStore()

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
    setSaving(true); setError(null)
    try {
      await updateDictionary(t, r)
      setTerm(''); setReplacement('')
      toast.success('Entry saved')
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
    <div className="flex flex-col h-full overflow-hidden">
      <div className="flex-1 min-h-0 overflow-hidden px-8 pt-7 pb-8 flex flex-col gap-6">

        {/* Hero */}
        <div className="flex items-center justify-between gap-4 pb-5 border-b border-[var(--border-soft)]">
          <div className="flex items-center gap-[14px]">
            <div className="w-9 h-9 rounded-[var(--r-lg)] bg-[var(--accent-soft)] text-[var(--accent)] flex items-center justify-center flex-shrink-0">
              <BookOpen size={18} strokeWidth={2} />
            </div>
            <div>
              <h1 className="text-[18px] font-bold tracking-[-0.025em] text-[var(--fg)] leading-[1.1] m-0">Dictionary</h1>
              <p className="text-[12px] text-[var(--muted)] mt-[3px] m-0">Custom phonetics and word replacements.</p>
            </div>
          </div>
        </div>

        {/* Notices */}
        <AnimatePresence>
          {error && (
            <motion.div key="dict-error" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
              <div className="flex items-center gap-[10px] px-[14px] py-[10px] rounded-[var(--r-lg)] text-[12px] leading-[1.4] flex-shrink-0 text-[var(--fg-2)]" style={{ background: 'var(--danger-soft)', border: '1px solid oklch(from var(--danger) l c h / 0.30)' }}>
                <AlertCircle size={13} strokeWidth={2} className="flex-shrink-0 text-[var(--danger)]" />
                <span className="flex-1">{error}</span>
                <button type="button" className="ml-auto text-[var(--muted)] bg-transparent border-none cursor-pointer px-[2px] text-[15px] leading-none rounded-[var(--r-xs)] flex-shrink-0 opacity-60 hover:opacity-100 transition-opacity" onClick={() => setError(null)}>
                  <X size={13} strokeWidth={2} />
                </button>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Quick Addition */}
        <div className="bg-[var(--panel)] border border-[var(--border)] rounded-[var(--r-xl)] px-[22px] py-5 flex-shrink-0">
          <div className="flex items-center gap-[7px] mb-4">
            <Plus size={14} strokeWidth={2} className="text-[var(--accent)]" />
            <span className="text-[10px] font-bold uppercase tracking-[0.08em] text-[var(--muted)]">Quick Addition</span>
          </div>
          <div className="grid grid-cols-[1fr_1fr_auto] gap-[14px] items-end">
            <div className="flex flex-col gap-[6px]">
              <label className="text-[10px] font-bold uppercase tracking-[0.06em] text-[var(--muted)]">Trigger Word</label>
              <Input
                value={term}
                onChange={(e) => setTerm(e.target.value)}
                placeholder="e.g. teh, gonna"
                disabled={saving}
              />
            </div>
            <div className="flex flex-col gap-[6px]">
              <label className="text-[10px] font-bold uppercase tracking-[0.06em] text-[var(--muted)]">Corrected Text</label>
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
        <div className="flex flex-col flex-1 min-h-0 gap-[14px] overflow-hidden">
          <div className="flex items-center justify-between flex-shrink-0">
            <h2 className="text-[15px] font-bold tracking-[-0.015em] text-[var(--fg)] m-0">Vocabulary Engine</h2>
          </div>

          <div className="flex-1 min-h-0 border border-[var(--border)] rounded-[var(--r-xl)] bg-[var(--bg)] overflow-hidden flex flex-col">
            <div className="flex-1 min-h-0 overflow-y-auto">
              {dictionary.length === 0 ? (
                <div className="flex flex-col items-center gap-2 py-8 px-4 text-center">
                  <div className="w-9 h-9 rounded-[var(--r-lg)] bg-[var(--surface)] border border-[var(--border-soft)] flex items-center justify-center text-[var(--muted)] opacity-80">
                    <BookOpen size={16} strokeWidth={1.5} />
                  </div>
                  <p className="text-[12px] text-[var(--muted)] max-w-[240px] leading-[1.5] m-0">No entries yet. Add your first correction above.</p>
                </div>
              ) : (
                <table className="w-full border-collapse text-left">
                  <thead>
                    <tr className="bg-[var(--panel)] border-b border-[var(--border)]">
                      <th className="px-4 py-[11px] text-[10px] font-bold uppercase tracking-[0.06em] text-[var(--muted)] whitespace-nowrap w-[30%]">Input Trigger</th>
                      <th className="px-4 py-[11px] text-[10px] font-bold uppercase tracking-[0.06em] text-[var(--muted)] whitespace-nowrap w-[30%]">Output Correction</th>
                      <th className="px-4 py-[11px] text-[10px] font-bold uppercase tracking-[0.06em] text-[var(--muted)] whitespace-nowrap w-[100px] text-center">Hits</th>
                      <th className="px-4 py-[11px] text-[10px] font-bold uppercase tracking-[0.06em] text-[var(--muted)] whitespace-nowrap w-[120px] text-right">Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    <AnimatePresence initial={false}>
                      {dictionary.map((entry) => (
                        <motion.tr
                          key={entry.id}
                          className="border-b border-[var(--border-soft)] last:border-none transition-colors duration-[var(--t-fast)] hover:bg-[var(--surface)]"
                          initial={{ opacity: 0 }}
                          animate={{ opacity: 1 }}
                          exit={{ opacity: 0, height: 0 }}
                          transition={{ duration: 0.15 }}
                          layout
                        >
                          {editId === entry.id ? (
                            <>
                              <td className="px-4 py-[14px] text-[13px] text-[var(--fg)]">
                                <Input value={editTerm} onChange={(e) => setEditTerm(e.target.value)} disabled={editSaving} className="h-[30px]! text-[12px]! px-2!" />
                              </td>
                              <td className="px-4 py-[14px] text-[13px] text-[var(--fg)]">
                                <Input value={editReplacement} onChange={(e) => setEditReplacement(e.target.value)} disabled={editSaving} onKeyDown={(e) => { if (e.key === 'Enter') commitEdit() }} className="h-[30px]! text-[12px]! px-2!" />
                              </td>
                              <td className="px-4 py-[14px] text-center">
                                <span className="text-[11px] font-bold text-[var(--muted)]">{entry.hits}</span>
                              </td>
                              <td className="px-4 py-[14px] text-right">
                                <div className="flex items-center justify-end gap-[2px]">
                                  <button type="button" className="w-7 h-7 flex items-center justify-center rounded-[var(--r-md)] border-none bg-transparent cursor-pointer text-[var(--muted)] transition-[background,color] duration-[var(--t-fast)]" style={{}} onClick={commitEdit} disabled={editSaving} onMouseOver={e => { (e.currentTarget as HTMLElement).style.background = 'color-mix(in srgb, var(--success) 12%, transparent)'; (e.currentTarget as HTMLElement).style.color = 'var(--success)' }} onMouseOut={e => { (e.currentTarget as HTMLElement).style.background = ''; (e.currentTarget as HTMLElement).style.color = '' }}>
                                    <Check size={14} strokeWidth={2.5} />
                                  </button>
                                  <button type="button" className="w-7 h-7 flex items-center justify-center rounded-[var(--r-md)] border-none bg-transparent cursor-pointer text-[var(--muted)] transition-[background,color] duration-[var(--t-fast)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)]" onClick={cancelEdit}>
                                    <X size={14} strokeWidth={2} />
                                  </button>
                                </div>
                              </td>
                            </>
                          ) : (
                            <>
                              <td className="px-4 py-[14px] text-[13px] text-[var(--fg)]">
                                <div className="flex items-center gap-[10px]">
                                  <Mic size={13} strokeWidth={1.75} className="text-[var(--muted)] flex-shrink-0" />
                                  <span className="text-[13px] font-medium text-[var(--fg)]">{entry.term}</span>
                                </div>
                              </td>
                              <td className="px-4 py-[14px] text-[13px] text-[var(--fg)]">
                                <span className="font-mono text-[12px] text-[var(--accent)] bg-[var(--accent-soft)] px-2 py-[2px] rounded-[var(--r-sm)]" style={{ border: '1px solid color-mix(in srgb, var(--accent) 20%, transparent)' }}>{entry.replacement}</span>
                              </td>
                              <td className="px-4 py-[14px] text-center">
                                <span className="text-[11px] font-bold text-[var(--muted)]">{entry.hits}</span>
                              </td>
                              <td className="px-4 py-[14px] text-right">
                                <div className="flex items-center justify-end gap-[2px]">
                                  <button type="button" className="w-7 h-7 flex items-center justify-center rounded-[var(--r-md)] border-none bg-transparent cursor-pointer text-[var(--muted)] transition-[background,color] duration-[var(--t-fast)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)]" onClick={() => startEdit(entry.id, entry.term, entry.replacement)}>
                                    <Pencil size={14} strokeWidth={1.75} />
                                  </button>
                                  <button type="button" className="w-7 h-7 flex items-center justify-center rounded-[var(--r-md)] border-none bg-transparent cursor-pointer text-[var(--muted)] transition-[background,color] duration-[var(--t-fast)]" onMouseOver={e => { (e.currentTarget as HTMLElement).style.background = 'color-mix(in srgb, var(--danger) 12%, transparent)'; (e.currentTarget as HTMLElement).style.color = 'var(--danger)' }} onMouseOut={e => { (e.currentTarget as HTMLElement).style.background = ''; (e.currentTarget as HTMLElement).style.color = '' }} onClick={() => deleteDictionaryEntry(entry.id)}>
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
            </div>
          </div>
        </div>

      </div>
    </div>
  )
}
