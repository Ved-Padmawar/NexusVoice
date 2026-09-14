import { Fragment, useState, useEffect, useCallback, useMemo, useRef, memo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Languages } from 'lucide-react'
import { toast } from 'sonner'
import { COMMANDS } from '../../lib/commands'
import { extractErrorMessage } from '../../lib/errors'
import { SearchInput } from '@/components/ui/input'
import { SelectMenu, SelectMenuItem } from '@/components/ui/select-menu'
import { SettingRow } from '../../components/page'

type LanguageOption = {
  code: string
  name: string
  isSelected: boolean
}

type LanguageSettings = {
  supported: boolean
  options: LanguageOption[]
}

/** Matches `inference::language::AUTO`. */
const AUTO = 'auto'

/** `.nv-menu-item` height, in rem. */
const ROW_REM = 2.125

/** Renders nothing for an English-only model. */
export const LanguageSection = memo(function LanguageSection({ modelId }: {
  /** Active model id — a change refetches, since support is per-model. */
  modelId?: string | null
}) {
  const [options, setOptions] = useState<LanguageOption[]>([])
  const [supported, setSupported] = useState(false)
  const [selected, setSelected] = useState<string>('en')
  const [loading, setLoading] = useState(true)
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const searchRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    invoke<LanguageSettings>(COMMANDS.GET_LANGUAGE_OPTIONS)
      .then(res => {
        if (cancelled) return
        const opts = Array.isArray(res?.options) ? res.options : []
        setSupported(Boolean(res?.supported))
        setOptions(opts)
        const active = opts.find(o => o.isSelected)
        if (active) setSelected(active.code)
      })
      .catch(e => {
        if (cancelled) return
        setSupported(false)
        toast.error(extractErrorMessage(e, 'Could not load languages'))
      })
      .finally(() => { if (!cancelled) setLoading(false) })
    return () => { cancelled = true }
  }, [modelId])

  // Radix focuses the selected item on open; claim it for the search.
  useEffect(() => {
    if (!open) return
    const id = requestAnimationFrame(() => searchRef.current?.focus())
    return () => cancelAnimationFrame(id)
  }, [open])

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return options
    return options.filter(o =>
      o.code === AUTO ||
      o.name.toLowerCase().includes(q) ||
      o.code.toLowerCase().includes(q),
    )
  }, [options, query])

  const choose = useCallback(async (code: string) => {
    setOpen(false)
    if (code === selected) return
    const previous = selected
    setSelected(code)
    try {
      await invoke<void>(COMMANDS.SET_LANGUAGE, { code })
    } catch (e) {
      setSelected(previous)
      toast.error(extractErrorMessage(e, 'Could not set language'))
    }
  }, [selected])

  if (!supported) return null

  const currentLabel = options.find(o => o.code === selected)?.name ?? 'English'

  return (
    <SettingRow
      title="Language"
      description="The language you dictate in. Auto-detect can mix languages mid-sentence."
      field
    >
      <div className="min-w-0 flex-1">
        <SelectMenu
          value={selected}
          onValueChange={(v) => void choose(v)}
          open={open}
          onOpenChange={(o) => { setOpen(o); if (!o) setQuery('') }}
          disabled={loading}
          icon={<Languages strokeWidth={2} />}
          label={currentLabel}
          display={currentLabel}
          // Pinned to the unfiltered list so typing doesn't resize the panel.
          panelStyle={{ height: `min(20rem, ${3.6 + options.length * ROW_REM}rem)`, maxHeight: 'none' }}
          header={
            <div className="shrink-0 border-b border-border-soft p-2">
              <SearchInput
                ref={searchRef}
                value={query}
                onChange={e => setQuery(e.target.value)}
                // Radix Select consumes printable keys for its own typeahead.
                onKeyDown={e => { if (e.key !== 'Escape') e.stopPropagation() }}
                placeholder="Search languages"
                aria-label="Search languages"
                inputClassName="nv-input--sm"
              />
            </div>
          }
        >
          {visible.length === 0 && (
            <p className="px-2.5 py-3 text-[12.5px] text-muted">No languages match.</p>
          )}
          {visible.map(opt => (
            <Fragment key={opt.code}>
              <SelectMenuItem value={opt.code}>{opt.name}</SelectMenuItem>
              {opt.code === AUTO && visible.length > 1 && <div className="nv-menu-sep" />}
            </Fragment>
          ))}
        </SelectMenu>
      </div>
    </SettingRow>
  )
})
