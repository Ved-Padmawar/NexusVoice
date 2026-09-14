import { useShallow } from 'zustand/react/shallow'
import { useRef, useEffect } from 'react'
import { useLocation } from 'react-router'
import { Palette, Info, SlidersHorizontal, AudioWaveform } from 'lucide-react'
import { useAppStore } from '../store/useAppStore'
import { SETTINGS_TABS, type SettingsTab } from '../lib/routes'
import { SCROLLER_SELECTOR } from '../lib/hooks'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { PageHeader, StickyBar } from '../components/page'
import { AppearanceTab } from './settings/AppearanceTab'
import { AboutTab } from './settings/AboutTab'
import { GeneralTab } from './settings/GeneralTab'
import { VoiceTab } from './settings/VoiceTab'

const SUBTITLE: Record<SettingsTab, string> = {
  general: 'Your microphone, dictation language, formatting and hotkeys.',
  voice: 'The speech model that turns your voice into text, on this computer.',
  appearance: 'How the window and the recording pill look.',
  about: 'Version, updates and this computer.',
}

export function Settings() {
  const { activeSettingsTab, setActiveSettingsTab } = useAppStore(useShallow(s => ({
    activeSettingsTab: s.activeSettingsTab,
    setActiveSettingsTab: s.setActiveSettingsTab,
  })))
  const location = useLocation()
  const rootRef = useRef<HTMLDivElement>(null)

  const initialLocationState = useRef(location.state)
  useEffect(() => {
    const requested = (initialLocationState.current as { tab?: string } | null)?.tab
    const validTabs = Object.values(SETTINGS_TABS) as string[]
    if (requested && validTabs.includes(requested)) {
      setActiveSettingsTab(requested as SettingsTab)
    }
  }, [setActiveSettingsTab])

  // Normalize stale persisted values ('pill' → 'appearance', where the pill
  // themes now live; 'shortcuts' → 'general', which absorbed them).
  const tab: SettingsTab =
    activeSettingsTab === ('pill' as SettingsTab)
      ? 'appearance'
      : activeSettingsTab === ('shortcuts' as SettingsTab)
        ? 'general'
        : activeSettingsTab

  const setTab = (v: string) => {
    const scroller = rootRef.current?.closest<HTMLElement>(SCROLLER_SELECTOR)
    if (scroller) scroller.scrollTop = 0
    setActiveSettingsTab(v as SettingsTab)
  }

  return (
    <div ref={rootRef} className="nv-page">
      <PageHeader title="Settings" description={SUBTITLE[tab]} />

      <Tabs value={tab} onValueChange={setTab}>
        <StickyBar>
          <TabsList aria-label="Settings sections">
            <TabsTrigger value="general"><SlidersHorizontal strokeWidth={1.9} />General</TabsTrigger>
            <TabsTrigger value="voice"><AudioWaveform strokeWidth={1.9} />Voice</TabsTrigger>
            <TabsTrigger value="appearance"><Palette strokeWidth={1.9} />Appearance</TabsTrigger>
            <TabsTrigger value="about"><Info strokeWidth={1.9} />About</TabsTrigger>
          </TabsList>
        </StickyBar>

        <TabsContent value="general"><GeneralTab /></TabsContent>
        <TabsContent value="voice"><VoiceTab /></TabsContent>
        <TabsContent value="appearance"><AppearanceTab /></TabsContent>
        <TabsContent value="about"><AboutTab /></TabsContent>
      </Tabs>
    </div>
  )
}
