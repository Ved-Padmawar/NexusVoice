import { memo, useEffect } from 'react'
import { MicrophoneSection } from './MicrophoneSection'
import { LanguageSection } from './LanguageSection'
import { HotkeySection } from './HotkeySection'
import { TextInjectionSection } from './TextInjectionSection'
import { FormattingToggle } from '../../components/FormattingToggle'
import { LiveTranscriptToggle } from '../../components/LiveTranscriptToggle'
import { Section, SettingGroup } from '../../components/page'
import { useAppStore } from '../../store/useAppStore'

/** Input first: it is what the rest of the app assumes is already right. */
export const GeneralTab = memo(function GeneralTab() {
  // Language support is per-model — the picker hides itself for English-only.
  const modelId = useAppStore((s) => s.selectedModel)
  const refreshModelInfo = useAppStore((s) => s.refreshModelInfo)

  // Settings can open straight here, before anything resolved the model.
  useEffect(() => { void refreshModelInfo() }, [refreshModelInfo])

  return (
    <>
      <Section title="Input" description="Where your voice comes from, and what language it speaks.">
        <SettingGroup>
          <MicrophoneSection />
          <LanguageSection modelId={modelId} />
        </SettingGroup>
      </Section>

      <Section title="Transcription" description="What happens to your words between speaking and pasting.">
        <SettingGroup>
          <FormattingToggle />
          <LiveTranscriptToggle />
        </SettingGroup>
      </Section>

      <Section title="Hotkeys" description="Global shortcuts. They work in any app, even with this window closed.">
        <HotkeySection />
      </Section>

      <TextInjectionSection />
    </>
  )
})
