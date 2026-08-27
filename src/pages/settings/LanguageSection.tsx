import { useState, useEffect, useCallback, useMemo, useRef, memo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { motion, AnimatePresence } from 'framer-motion'
import { Select } from 'radix-ui'
import { Languages, Check, ChevronDown, Search } from 'lucide-react'
import { toast } from 'sonner'
import { COMMANDS } from '../../lib/commands'
import { extractErrorMessage } from '../../lib/errors'

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
        onSupportedChange?.(Boolean(res?.supported))
      })
      .catch(e => {
        if (cancelled) return
        setSupported(false)
        onSupportedChange?.(false)
        toast.error(extractErrorMessage(e, 'Could not load languages'))
      })
      .finally(() => { if (!cancelled) setLoading(false) })
    return () => { cancelled = true }
  }, [modelId, onSupportedChange])

  // Radix focuses the selected item on open; claim it for the search.
  useEffect(() => {
    if (!open) return
    const id = requestAnimationFrame(() => searchRef.current?.focus())
    return () => cancelAnimationFrame(id)
  }, [open])

  // Pinned to the unfiltered list so typing doesn't resize the panel.
  const listHeight = 3 + options.length * 2.25

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

  // Inert for an English-only model.
  if (!supported) return null

  return (
    <div className="flex flex-col gap-4">
      <div>
        <p className="text-[12px] font-semibold text-(--fg-2) tracking-[-0.01em] mb-1">Language</p>
        <p className="text-[12px] text-muted-foreground">
          The language you dictate in. Auto-detect can mix languages mid-sentence.
        </p>
      </div>

      <Select.Root
        value={selected}
        onValueChange={(v) => void choose(v)}
        open={open}
        onOpenChange={(o) => { setOpen(o); if (!o) setQuery('') }}
      >
        <Select.Trigger asChild disabled={loading}>
          <button
            type="button"
            aria-label={currentLabel}
            className={`relative flex min-w-0 items-center w-full h-9 pl-8 pr-8 rounded-(--r-md) bg-(--surface) border text-[12px] text-(--fg) cursor-pointer text-left transition-[border-color] duration-(--t-fast) focus:outline-none disabled:opacity-50 ${open ? 'border-(--accent)' : 'border-(--border-soft) hover:border-(--border)'}`}
          >
            <Languages size={14} strokeWidth={2} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-(--accent) pointer-events-none" />
            <span className="truncate">
              <Select.Value>{currentLabel}</Select.Value>
            </span>
            <motion.span
              className="absolute right-2.5 top-1/2 -translate-y-1/2 text-(--fg-2) pointer-events-none flex"
              animate={{ rotate: open ? 180 : 0 }}
              transition={{ duration: 0.18 }}
            >
              <ChevronDown size={14} strokeWidth={2} />
            </motion.span>
          </button>
        </Select.Trigger>

        <AnimatePresence>
          {open && (
            <Select.Portal forceMount>
              <Select.Content asChild position="popper" sideOffset={4}>
                <motion.div
                  initial={{ opacity: 0, y: -4, scale: 0.98 }}
                  animate={{ opacity: 1, y: 0, scale: 1 }}
                  exit={{ opacity: 0, y: -4, scale: 0.98 }}
                  transition={{ duration: 0.14, ease: 'easeOut' }}
                  style={{ height: `min(18rem, ${listHeight}rem)` }}
                  className="z-50 flex flex-col w-(--radix-select-trigger-width) overflow-hidden rounded-(--r-lg) bg-(--panel) border border-(--border) shadow-(--shadow-lg)"
                >
                  <div className="relative shrink-0 border-b border-(--border-soft) p-2">
                    <Search size={13} strokeWidth={2.25} className="absolute left-4 top-1/2 -translate-y-1/2 text-(--fg-2) pointer-events-none" />
                    <input
                      ref={searchRef}
                      value={query}
                      onChange={e => setQuery(e.target.value)}
                      // Radix Select consumes printable keys for its own typeahead.
                      onKeyDown={e => { if (e.key !== 'Escape') e.stopPropagation() }}
                      placeholder="Search languages"
                      aria-label="Search languages"
                      className="w-full h-8 pl-7 pr-2 rounded-(--r-sm) bg-(--surface) border border-(--border-soft) text-[12px] text-(--fg) placeholder:text-muted-foreground outline-none focus:border-(--accent)"
                    />
                  </div>

                  <Select.Viewport
                    className="flex-1 min-h-0 overflow-x-hidden"
                    // Radix inlines `overflow: hidden auto` here, beating the class.
                    style={{ overflowY: 'auto', overscrollBehavior: 'none' }}
                  >
                    {visible.length === 0 && (
                      <p className="px-3.5 py-3 text-[12px] text-muted-foreground">No languages match.</p>
                    )}
                    {visible.map((opt, i) => {
                      const active = opt.code === selected
                      return (
                        <Select.Item
                          key={opt.code}
                          value={opt.code}
                          className={`flex items-center h-9 px-3.5 text-[12px] text-(--fg) cursor-pointer outline-none select-none data-highlighted:bg-(--surface) ${i === visible.length - 1 ? 'rounded-b-(--r-lg)' : ''} ${opt.code === AUTO ? 'border-b border-(--border-soft)' : ''}`}
                        >
                          <span className={`flex-1 truncate ${active ? 'font-semibold text-(--accent)' : ''}`}>
                            <Select.ItemText>{opt.name}</Select.ItemText>
                          </span>
                          <Select.ItemIndicator className="shrink-0 ml-2 text-(--accent)">
                            <Check size={13} strokeWidth={2.5} />
                          </Select.ItemIndicator>
                        </Select.Item>
                      )
                    })}
                  </Select.Viewport>
                </motion.div>
              </Select.Content>
            </Select.Portal>
          )}
        </AnimatePresence>
      </Select.Root>
    </div>
  )
})
