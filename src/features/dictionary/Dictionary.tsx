import { useEffect, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import {
  BookOpenTextIcon, CaretDownIcon, CheckIcon, MagnifyingGlassIcon, PencilSimpleIcon, PlusIcon, TrashIcon,
} from '@phosphor-icons/react'
import { useDictionary, useUpdateDictionary, useDeleteDictionaryEntry } from './queries'
import { cn } from '../../shared/utils'
import { Button } from '@/shared/ui/Button'
import { Kbd } from '@/shared/ui/Kbd'
import { MenuItem, Popover } from '@/shared/ui/Popover'
import { TextInput } from '@/shared/ui/TextInput'
import { SectionState } from '../../shared/ui/SectionState'

type SortKey = 'hits' | 'az' | 'recent'
const SORTS: { value: SortKey; label: string }[] = [
  { value: 'hits', label: 'Most used' },
  { value: 'az', label: 'A to Z' },
  { value: 'recent', label: 'Newest first' },
]

const NEW = 'new'
type EditId = number | typeof NEW | null

const cell = 'h-10.5 px-2.5 border-b border-line-soft first:pl-7 last:pr-4.5'
const headCell = 'sticky top-0 z-2 h-8.5 bg-bg px-2.5 text-left text-2xs font-semibold tracking-widest text-muted uppercase shadow-[inset_0_-1px_0_var(--border)] first:pl-7 last:pr-4.5'
const iconAction = 'grid size-7 cursor-pointer place-items-center rounded-[7px] text-muted transition-colors'

function DictionarySkeleton() {
  return (
    <div className="flex flex-col">
      {[0, 1, 2, 3, 4].map(i => (
        <div key={i} className="flex items-center gap-4 border-b border-line-soft px-7 py-3.5">
          <div className="h-3.5 w-[28%] animate-pulse rounded bg-surface" />
          <div className="h-3.5 w-[28%] animate-pulse rounded bg-surface" />
          <div className="ml-auto h-3.5 w-8 animate-pulse rounded bg-surface" />
        </div>
      ))}
    </div>
  )
}

function Arrow() {
  return (
    <svg className="pointer-events-none absolute top-1/2 right-2 h-4 w-10 -translate-y-1/2 text-muted" viewBox="0 0 48 16" aria-hidden="true">
      <path d="M2 8h44M41.5 3.5 46 8l-4.5 4.5" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}

export function Dictionary() {
  const { data: dictionary = [], status, error, refetch } = useDictionary()
  const updateDictionary = useUpdateDictionary()
  const deleteDictionaryEntry = useDeleteDictionaryEntry()

  const [query, setQuery] = useState('')
  const [sort, setSort] = useState<SortKey>('hits')
  const [sortOpen, setSortOpen] = useState(false)
  const sortRef = useRef<HTMLButtonElement>(null)
  const searchRef = useRef<HTMLInputElement>(null)

  const [editId, setEditId] = useState<EditId>(null)
  const [editTerm, setEditTerm] = useState('')
  const [editReplacement, setEditReplacement] = useState('')
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== '/' || e.ctrlKey || e.metaKey || e.altKey) return
      if ((e.target as HTMLElement).matches('input, textarea, select')) return
      e.preventDefault()
      searchRef.current?.focus()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [])

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase()
    return dictionary
      .filter(d => d.id === editId || `${d.term} ${d.replacement}`.toLowerCase().includes(q))
      .sort((a, b) => sort === 'az' ? a.term.localeCompare(b.term) : sort === 'recent' ? b.id - a.id : b.hits - a.hits)
  }, [dictionary, query, sort, editId])

  const startEdit = (id: EditId, term = '', replacement = '') => {
    setEditId(id); setEditTerm(term); setEditReplacement(replacement)
  }
  const cancelEdit = () => startEdit(null)

  const canSave = !saving && !!editTerm.trim() && !!editReplacement.trim()
  const commitEdit = async () => {
    const t = editTerm.trim(), r = editReplacement.trim()
    if (!t || !r) return
    setSaving(true)
    try {
      await updateDictionary.mutateAsync({
        term: t,
        replacement: r,
        previousTerm: editId === NEW ? undefined : dictionary.find(d => d.id === editId)?.term,
      })
      if (editId === NEW) { toast.success('Entry saved'); setQuery('') }
      cancelEdit()
    } catch {
      // The mutation reports the error; preserve the edit for retry.
    } finally { setSaving(false) }
  }

  const onEditKey = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') { e.preventDefault(); void commitEdit() }
    if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); cancelEdit() }
  }

  const editInput = 'h-8 rounded-[7px] bg-panel px-2.5 text-sm ring-line'
  const editing = (id: EditId, hits: string, key: string) => (
    <tr key={key} className="bg-accent-soft transition-opacity duration-150 starting:opacity-0 [&>td]:h-12 [&>td]:bg-accent-soft">
      <td className={cn(cell, 'relative pr-12')}>
        <input
          autoFocus
          aria-label="When it hears"
          placeholder="Heard word"
          autoComplete="off"
          value={editTerm}
          disabled={saving}
          onChange={e => setEditTerm(e.target.value)}
          onKeyDown={onEditKey}
          className={cn('w-full outline-none ring-1 ring-inset focus:ring-[1.5px] focus:ring-accent disabled:opacity-50', editInput)}
        />
        <Arrow />
      </td>
      <td className={cell}>
        <input
          aria-label="Write instead"
          placeholder="Replacement"
          autoComplete="off"
          value={editReplacement}
          disabled={saving}
          onChange={e => setEditReplacement(e.target.value)}
          onKeyDown={onEditKey}
          className={cn('w-full font-semibold text-accent-ink outline-none ring-1 ring-inset focus:ring-[1.5px] focus:ring-accent disabled:opacity-50', editInput)}
        />
      </td>
      <td className={cn(cell, 'text-right text-xs text-muted tabular-nums')}>{hits}</td>
      <td className={cell}>
        <div className="flex justify-end gap-2">
          <Button size="sm" variant="outline" onClick={cancelEdit}>Cancel</Button>
          <Button size="sm" variant="primary" disabled={!canSave} onClick={() => void commitEdit()}>{id === NEW ? 'Add' : 'Save'}</Button>
        </div>
      </td>
    </tr>
  )

  const showEmpty = dictionary.length === 0 && editId !== NEW

  return (
    <div className="relative flex min-h-0 flex-col overflow-hidden pt-3">
      <div className="flex shrink-0 items-center gap-2.5 px-7">
        <h1 className="text-2xl font-bold tracking-[-0.03em] text-fg">Dictionary</h1>
        {dictionary.length > 0 && (
          <span className="rounded-md px-1.75 py-0.5 text-sm font-semibold text-muted tabular-nums ring-1 ring-inset ring-line-soft">{dictionary.length}</span>
        )}
      </div>

      <div className="flex shrink-0 items-center gap-2.5 px-7 pt-5 pb-3.5">
        <TextInput
          ref={searchRef}
          type="search"
          value={query}
          onChange={e => setQuery(e.target.value)}
          onKeyDown={e => { if (e.key === 'Escape') { setQuery(''); e.currentTarget.blur() } }}
          placeholder="Find a word…"
          aria-label="Search the dictionary"
          autoComplete="off"
          icon={<MagnifyingGlassIcon size={14} />}
          trailing={<Kbd>/</Kbd>}
          wrapperClassName="w-80"
          className="h-8.5"
        />
        <button
          ref={sortRef}
          type="button"
          aria-haspopup="listbox"
          aria-expanded={sortOpen}
          onClick={() => setSortOpen(o => !o)}
          className="flex h-8.5 cursor-pointer items-center gap-2 rounded-lg bg-field pr-2.5 pl-3 text-sm text-fg-2 ring-1 ring-inset ring-line-soft hover:ring-line"
        >
          <span className="text-muted">Sort:</span>{SORTS.find(s => s.value === sort)?.label}
          <CaretDownIcon size={12} weight="bold" className={cn('text-muted transition-[rotate] duration-150', sortOpen && 'rotate-180')} />
        </button>
        <Popover open={sortOpen} onOpenChange={setSortOpen} anchorRef={sortRef} role="listbox" aria-label="Sort words" className="w-44 rounded-[10px]">
          {SORTS.map(s => (
            <MenuItem
              key={s.value}
              role="option"
              aria-selected={s.value === sort}
              onClick={() => { setSort(s.value); setSortOpen(false) }}
              className={cn('justify-between text-xs', s.value === sort && 'font-semibold text-accent-ink')}
            >
              {s.label}
              <CheckIcon size={12} weight="bold" className={s.value === sort ? '' : 'invisible'} />
            </MenuItem>
          ))}
        </Popover>
        <Button variant="tinted" size="lg" className="h-8.5 px-3.5 text-sm" onClick={() => startEdit(NEW)}>
          <PlusIcon size={14} weight="bold" />Add word
        </Button>
      </div>

      <div className="page-scroll min-h-0 flex-1 overflow-y-auto overscroll-none">
        <SectionState status={status} error={error?.message} onRetry={refetch} skeleton={<DictionarySkeleton />}>
          {showEmpty ? (
            // Centred on the page, not the list area under the toolbar.
            <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-3 px-6 text-center">
              <span className="grid size-14 place-items-center rounded-2xl bg-accent-soft text-accent-ink">
                <BookOpenTextIcon size={28} weight="duotone" />
              </span>
              <p className="text-md font-semibold text-fg">No corrections yet</p>
              <p className="max-w-72 text-sm text-muted text-balance">Add the names and acronyms NexusVoice gets wrong.</p>
            </div>
          ) : (
            <table className="w-full table-fixed border-separate border-spacing-0 text-sm">
              <colgroup><col className="w-[38%]" /><col className="w-[38%]" /><col className="w-20" /><col /></colgroup>
              <thead>
                <tr>
                  <th scope="col" className={headCell}>When it hears</th>
                  <th scope="col" className={headCell}>Write instead</th>
                  <th scope="col" className={cn(headCell, 'text-right')}>Uses</th>
                  <th scope="col" aria-label="Actions" className={headCell} />
                </tr>
              </thead>
              <tbody>
                {editId === NEW && editing(NEW, '—', NEW)}
                {shown.map(entry => entry.id === editId ? editing(entry.id, String(entry.hits), String(entry.id)) : (
                  <tr
                    key={entry.id}
                    className="group cursor-pointer [&>td]:transition-colors [&:hover>td]:bg-[color-mix(in_oklab,var(--fg)_4%,var(--bg))]"
                    onClick={() => startEdit(entry.id, entry.term, entry.replacement)}
                  >
                    <td className={cn(cell, 'relative pr-12')}>
                      <span className="block truncate text-fg-2">{entry.term}</span>
                      <Arrow />
                    </td>
                    <td className={cell}><span className="block truncate font-semibold text-fg">{entry.replacement}</span></td>
                    <td className={cn(cell, 'text-right text-xs text-muted tabular-nums')}>{entry.hits}</td>
                    <td className={cell}>
                      <div className="flex justify-end gap-0.5 opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100">
                        <button
                          type="button"
                          aria-label={`Edit ${entry.term}`}
                          className={cn(iconAction, 'hover:bg-accent-soft hover:text-accent-ink')}
                          onClick={e => { e.stopPropagation(); startEdit(entry.id, entry.term, entry.replacement) }}
                        >
                          <PencilSimpleIcon size={15} />
                        </button>
                        <button
                          type="button"
                          aria-label={`Delete ${entry.term}`}
                          className={cn(iconAction, 'hover:bg-danger-soft hover:text-danger')}
                          onClick={e => { e.stopPropagation(); deleteDictionaryEntry.mutate(entry.id) }}
                        >
                          <TrashIcon size={15} />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
                {shown.length === 0 && editId !== NEW && (
                  <tr><td colSpan={4} className="h-30 text-center text-muted">No word matches “{query.trim()}”</td></tr>
                )}
              </tbody>
            </table>
          )}
        </SectionState>
      </div>
    </div>
  )
}
