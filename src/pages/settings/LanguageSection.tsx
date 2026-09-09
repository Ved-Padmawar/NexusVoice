import { useState, useEffect, useCallback, useMemo, useRef, memo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Select } from 'radix-ui'
import { Languages, Check, ChevronDown, Search } from 'lucide-react'
import { toast } from 'sonner'
import { COMMANDS } from '../../lib/commands'
import { extractErrorMessage } from '../../lib/errors'
import { SELECT_CONTENT, SELECT_ITEM, SELECT_TRIGGER } from './selectStyles'

type LanguageOption = {
  code: string
  name: string
  isSelected: boolean
}

type LanguageSettings = {
  supported: boolean
  options: LanguageOption[]
}

type Props = {
  /** Active model id — a change refetches, since support is per-model. */
  modelId?: string | null
  onSupportedChange?: (supported: boolean) => void
}

/** Matches `inference::language::AUTO`. */
const AUTO = 'auto'

export const LanguageSection = memo(function LanguageSection({ modelId, onSupportedChange }: Props) {
  const [options, setOptions] = useState<LanguageOption[]>([])
  const [supported, setSupported] = useState(false)
  const [selected, setSelected] = useState<string>('en')
  const [loading, setLoading] = useState(true)
  /** True once a fetch has returned an answer, so `supported` is meaningful. */
  const [settled, setSettled] = useState(false)
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const searchRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    // `modelId` is null for a moment whenever `refreshModelInfo` runs before
    // the catalog resolves. Refetching on that transient is what made the
    // picker disappear until an unrelated re-render brought it back.
    if (modelId === null) return
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
        onSupportedChange?.(Boolean(res?.supported))
      })
      .catch(e => {
        if (cancelled) return
        setSupported(false)
        onSupportedChange?.(false)
        toast.error(extractErrorMessage(e, 'Could not load languages'))
      })
      .finally(() => { if (!cancelled) { setLoading(false); setSettled(true) } })
    return () => { cancelled = true }
  }, [modelId, onSupportedChange])

  // Radix focuses the selected item on open; claim it for the search.
  useEffect(() => {
    if (!open) return
    const id = requestAnimationFrame(() => searchRef.current?.focus())
    return () => cancelAnimationFrame(id)
  }, [open])

  // Pinned to the unfiltered list so typing doesn't resize the panel.
  const listHeight = 3.25 + options.length * 2.25

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

  const currentLabel = options.find(o => o.code === selected)?.name ?? 'English'

  // Inert for an English-only model, but only once a fetch has actually said
  // so. Before that, `supported` is just its initial `false` and hiding on it
  // would blank a picker that is really supported.
  if (settled && !supported) return null
  if (!settled) return null

  return (
    <div className="flex min-w-0 max-w-96 flex-col gap-2">
      <span className="text-[11px] text-(--muted)">Dictation language</span>

      <Select.Root
        value={selected}
        onValueChange={(v) => void choose(v)}
        open={open}
        onOpenChange={(o) => { setOpen(o); if (!o) setQuery('') }}
      >
        <Select.Trigger asChild disabled={loading}>
          <button type="button" aria-label={currentLabel} className={`${SELECT_TRIGGER} w-full`}>
            <Languages size={13} strokeWidth={2} className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-(--on-soft)" />
            <span className="truncate">
              <Select.Value>{currentLabel}</Select.Value>
            </span>
            <ChevronDown
              size={13}
              strokeWidth={2}
              className="pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 text-(--muted) transition-transform duration-(--t-fast) group-data-[state=open]:rotate-180"
            />
          </button>
        </Select.Trigger>

        <Select.Portal>
          <Select.Content
            position="popper"
            sideOffset={5}
            className={`${SELECT_CONTENT} flex flex-col`}
            style={{ height: `min(18rem, ${listHeight}rem)` }}
          >
            <div className="relative shrink-0 border-b border-(--hairline) p-2">
              <Search size={12} strokeWidth={2.25} className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-(--faint)" />
              <input
                ref={searchRef}
                value={query}
                onChange={e => setQuery(e.target.value)}
                // Radix Select consumes printable keys for its own typeahead.
                onKeyDown={e => { if (e.key !== 'Escape') e.stopPropagation() }}
                placeholder="Search languages"
                aria-label="Search languages"
                className="field h-7 pl-7 text-[12px]"
              />
            </div>

            <Select.Viewport
              className="select-list min-h-0 flex-1"
              // Radix inlines `overflow: hidden auto` here, beating the class.
              style={{ overflowY: 'auto', overscrollBehavior: 'none' }}
            >
              {visible.length === 0 && (
                <p className="px-3 py-3 text-[12px] text-(--muted)">No languages match.</p>
              )}
              {visible.map((opt) => (
                <Select.Item
                  key={opt.code}
                  value={opt.code}
                  className={`${SELECT_ITEM} ${opt.code === AUTO ? 'border-b border-(--hairline)' : ''}`}
                >
                  <span className="min-w-0 flex-1 truncate">
                    <Select.ItemText>{opt.name}</Select.ItemText>
                  </span>
                  <Select.ItemIndicator className="ml-2 shrink-0 text-(--on-soft)">
                    <Check size={13} strokeWidth={2.5} />
                  </Select.ItemIndicator>
                </Select.Item>
              ))}
            </Select.Viewport>
          </Select.Content>
        </Select.Portal>
      </Select.Root>
    </div>
  )
})
