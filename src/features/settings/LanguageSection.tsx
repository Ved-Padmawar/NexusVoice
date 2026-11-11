import { useEffect, memo } from 'react'
import { TranslateIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { useLiveChoice } from '../../app/useLiveChoice'
import { Select } from '../../shared/ui/Select'
import { languageOptions, useLanguageOptions } from '../models/queries'
import { InputHint } from './InputHint'

/** Matches `models::language::AUTO`. */
const AUTO = 'auto'

const saveLanguage = (code: string) => commands.setLanguage(code)
const saveFailed = (e: unknown) => toast.error(extractErrorMessage(e, 'Could not set language'))

export const LanguageSection = memo(function LanguageSection() {
  const { data, error } = useLanguageOptions()
  const { pending, choose } = useLiveChoice(languageOptions, saveLanguage, saveFailed)

  useEffect(() => {
    if (error) toast.error(extractErrorMessage(error, 'Could not load languages'))
  }, [error])

  // Inert for an English-only model.
  if (!data?.supported) return null

  const options = data.options
  const selected = pending ? pending.value : options.find(o => o.isSelected)?.code ?? AUTO

  return (
    <div className="flex min-w-0 flex-col gap-2.5">
      <div>
        <p className="text-sm font-semibold text-fg">Language</p>
        <InputHint input="language">What you dictate in. Auto-detect handles mixed speech.</InputHint>
      </div>

      <Select
        value={selected}
        options={options.map(o => ({ value: o.code, label: o.name, dividerAfter: o.code === AUTO }))}
        onChange={(v) => { if (v !== selected) void choose(v) }}
        aria-label="Language"
        icon={<TranslateIcon size={14} weight="bold" />}
        searchPlaceholder={`Search ${options.length} languages`}
      />
    </div>
  )
})
