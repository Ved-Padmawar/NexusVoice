import { useState, useEffect, useCallback, memo } from 'react'
import { TranslateIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { Select } from '../../shared/ui/Select'
import type { LanguageOption } from '../../shared/types'

type Props = {
  /** Active model id — a change refetches, since support is per-model. */
  modelId?: string | null
  onSupportedChange?: (supported: boolean) => void
}

/** Matches `models::language::AUTO`. */
const AUTO = 'auto'

export const LanguageSection = memo(function LanguageSection({ modelId, onSupportedChange }: Props) {
  const [options, setOptions] = useState<LanguageOption[]>([])
  const [supported, setSupported] = useState(false)
  const [selected, setSelected] = useState<string>('en')
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    commands.getLanguageOptions()
      .then(res => {
        if (cancelled) return
        setSupported(res.supported)
        setOptions(res.options)
        const active = res.options.find(o => o.isSelected)
        if (active) setSelected(active.code)
        onSupportedChange?.(res.supported)
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

  const choose = useCallback(async (code: string) => {
    if (code === selected) return
    const previous = selected
    setSelected(code)
    try {
      await commands.setLanguage(code)
    } catch (e) {
      setSelected(previous)
      toast.error(extractErrorMessage(e, 'Could not set language'))
    }
  }, [selected])

  // Inert for an English-only model.
  if (!supported) return null

  return (
    <div className="flex min-w-0 flex-col gap-2.5">
      <div>
        <p className="text-sm font-semibold text-fg">Language</p>
        <p className="text-xs text-muted">What you dictate in. Auto-detect handles mixed speech.</p>
      </div>

      <Select
        value={selected}
        options={options.map(o => ({ value: o.code, label: o.name, dividerAfter: o.code === AUTO }))}
        onChange={(v) => void choose(v)}
        aria-label="Language"
        icon={<TranslateIcon size={14} weight="bold" />}
        disabled={loading}
        searchPlaceholder={`Search ${options.length} languages`}
      />
    </div>
  )
})
