import { memo } from 'react'
import { TranslateIcon } from '@phosphor-icons/react'
import { toast } from 'sonner'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { useSlice } from '../../app/mirror'
import { useChoice } from '../../app/useChoice'
import { Select } from '../../shared/ui/Select'
import { useKeptByRecording } from './queries'

/** Matches `models::language::AUTO`. */
const AUTO = 'auto'
const ENGLISH = 'en'
const ENGLISH_ONLY = [{ value: ENGLISH, label: 'English' }]

const saveLanguage = (code: string) => commands.setLanguage(code)
const saveFailed = (e: unknown) => toast.error(extractErrorMessage(e, 'Could not set language'))

/** What you dictate in, as a compact picker for the Voice toolbar. */
export const LanguageSection = memo(function LanguageSection() {
  const language = useSlice('settings', settings => settings.language)
  const { pending, choose } = useChoice('settings', 'language', saveLanguage, saveFailed)
  const kept = useKeptByRecording('language')

  const { supported } = language
  // An English-only model shows English as its value, not as a faint placeholder.
  const options = supported ? language.options.map(o => ({ value: o.code, label: o.name, dividerAfter: o.code === AUTO })) : ENGLISH_ONLY
  const selected = supported ? (pending ? pending.value : language.options.find(o => o.isSelected)?.code ?? AUTO) : ENGLISH

  let tip = 'Language: what you dictate in'
  if (!supported) tip = 'This model only understands English'
  else if (kept) tip = 'Saved. This recording keeps the previous one.'

  return (
    <span data-tip={tip} data-tip-pos="bottom" data-tip-align="end" className="block w-40 shrink-0">
      <Select
        value={selected}
        options={options}
        onChange={(v) => { if (v !== selected) choose(v) }}
        aria-label="Language"
        placeholder="Language"
        icon={<TranslateIcon size={14} weight="bold" />}
        searchPlaceholder={`Search ${options.length} languages`}
        disabled={!supported}
        className="h-8 rounded-lg text-xs font-semibold disabled:opacity-60"
      />
    </span>
  )
})
