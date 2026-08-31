import { memo, useCallback, useEffect, useState } from 'react'
import { MicrophoneSection } from './MicrophoneSection'
import { LanguageSection } from './LanguageSection'
import { HotkeySection } from './HotkeySection'
import { TextInjectionSection } from './TextInjectionSection'
import { FormattingToggle } from '../../components/FormattingToggle'
import { useAppStore } from '../../store/useAppStore'

/** Input first: it is what the rest of the app assumes is already right. */

export const GeneralTab = memo(function GeneralTab() {
  // Language support is per-model — the picker hides itself for English-only.
  const modelId = useAppStore((s) => s.selectedModel)
  const refreshModelInfo = useAppStore((s) => s.refreshModelInfo)
  const [langSupported, setLangSupported] = useState(false)
  const onLangSupportedChange = useCallback((v: boolean) => setLangSupported(v), [])

  // Settings can open straight here, before anything resolved the model.
  useEffect(() => { void refreshModelInfo() }, [refreshModelInfo])

  return (
    <div className="flex flex-col gap-4">
      {/* `overflow-visible` — both selects open panels past the card edge. */}
      <div
        className={`grid gap-4 rounded-(--r-lg) border border-(--border-soft) bg-(--panel) p-4 *:min-w-0 ${
          langSupported ? 'grid-cols-1 md:grid-cols-2' : 'grid-cols-1'
        }`}
      >
        <MicrophoneSection />
        <LanguageSection modelId={modelId} onSupportedChange={onLangSupportedChange} />
      </div>

      <FormattingToggle />

      <div className="rounded-(--r-lg) border border-(--border-soft) bg-(--panel) p-4">
        <HotkeySection />
      </div>

      <TextInjectionSection />
    </div>
  )
})
