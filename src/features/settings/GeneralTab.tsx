import { memo, useCallback, useState, type ReactNode } from 'react'
import { MicrophoneSection } from './MicrophoneSection'
import { LanguageSection } from './LanguageSection'
import { HotkeySection } from './HotkeySection'
import { TextInjectionSection } from './TextInjectionSection'
import { FormattingToggle } from './FormattingToggle'
import { LiveTranscriptToggle } from './LiveTranscriptToggle'
import { StartSoundToggle } from './StartSoundToggle'
import { useActiveModel } from '../models/queries'

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h2 className="mb-2 text-sm font-semibold text-fg-2">{title}</h2>
      {children}
    </section>
  )
}

/** Input first: it is what the rest of the app assumes is already right. */
export const GeneralTab = memo(function GeneralTab() {
  // Language support is per-model — the picker hides itself for English-only.
  const modelId = useActiveModel().data?.id ?? null
  const [langSupported, setLangSupported] = useState(false)
  const onLangSupportedChange = useCallback((v: boolean) => setLangSupported(v), [])

  return (
    <div className="flex flex-col gap-5">
      <Section title="Input">
        <div className={`grid gap-5 rounded-2xl bg-panel p-4 ring-1 ring-line-soft ring-inset ${langSupported ? 'grid-cols-2' : 'grid-cols-1'}`}>
          <MicrophoneSection />
          <LanguageSection modelId={modelId} onSupportedChange={onLangSupportedChange} />
        </div>
      </Section>

      <Section title="Output">
        <div className="grid grid-cols-2 gap-5">
          <FormattingToggle />
          <LiveTranscriptToggle />
          <div className="col-span-2">
            <StartSoundToggle />
          </div>
        </div>
      </Section>

      <Section title="Shortcuts">
        <HotkeySection />
      </Section>

      <TextInjectionSection />
    </div>
  )
})
