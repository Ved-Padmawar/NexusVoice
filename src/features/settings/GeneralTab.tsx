import { memo, type ReactNode } from 'react'
import { MicrophoneSection } from './MicrophoneSection'
import { KeepMicOpenToggle } from './KeepMicOpenToggle'
import { ModelUnloadSection } from './ModelUnloadSection'
import { HotkeySection } from './HotkeySection'
import { PasteModeSection } from './PasteModeSection'
import { SavedAudioSection } from './SavedAudioSection'
import { SettingsCard } from './SettingRow'
import { TextInjectionSection } from './TextInjectionSection'
import { FormattingToggle } from './FormattingToggle'
import { LiveTranscriptToggle } from './LiveTranscriptToggle'
import { StartSoundToggle } from './StartSoundToggle'

function Group({ title, children, className = '' }: { title: string; children: ReactNode; className?: string }) {
  return (
    <section className={`flex min-w-0 flex-col ${className}`}>
      <h2 className="mb-2 text-sm font-semibold text-fg-2">{title}</h2>
      {children}
    </section>
  )
}

/** Shortcuts across the top; below, what goes in (left) and what comes out (right). */
export const GeneralTab = memo(function GeneralTab() {
  return (
    <div className="flex flex-col gap-5">
      <Group title="Shortcuts">
        <HotkeySection />
      </Group>

      <div className="grid grid-cols-2 gap-5">
        <div className="flex min-w-0 flex-col gap-5">
          <Group title="Input" className="flex-1">
            <SettingsCard className="flex-1">
              <MicrophoneSection />
              <KeepMicOpenToggle />
              <ModelUnloadSection />
              <StartSoundToggle />
            </SettingsCard>
          </Group>
          <Group title="History">
            <SavedAudioSection />
          </Group>
        </div>

        <Group title="Output">
          <SettingsCard className="flex-1">
            <PasteModeSection />
            <FormattingToggle />
            <LiveTranscriptToggle />
          </SettingsCard>
        </Group>
      </div>

      <TextInjectionSection />
    </div>
  )
})
