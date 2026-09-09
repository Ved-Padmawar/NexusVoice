import { useShallow } from 'zustand/react/shallow'
import { useRef, useEffect } from 'react'
import { useLocation } from 'react-router'
import { SlidersHorizontal, Mic, Palette, Info } from 'lucide-react'
import { useAppStore } from '../store/useAppStore'
import { SETTINGS_TABS, type SettingsTab } from '../lib/routes'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { PageBar } from '../components/PageBar'
import { AppearanceTab } from './settings/AppearanceTab'
import { AboutTab } from './settings/AboutTab'
import { GeneralTab } from './settings/GeneralTab'
import { VoiceTab } from './settings/VoiceTab'
import { PillTab } from './settings/PillTab'
import { WaveformTab } from './settings/WaveformTab'

/** Tab order is the registry order (`SETTINGS_TABS`); keep the two in sync. */
const TABS: { value: SettingsTab; label: string; Icon: typeof Mic }[] = [
  { value: SETTINGS_TABS.GENERAL,    label: 'General',    Icon: SlidersHorizontal },
  { value: SETTINGS_TABS.VOICE,      label: 'Voice',      Icon: Mic },
  { value: SETTINGS_TABS.APPEARANCE, label: 'Appearance', Icon: Palette },
  { value: SETTINGS_TABS.ABOUT,      label: 'About',      Icon: Info },
]

/* One scroll container per tab. The clearance is a margin, not padding: padding
   sits inside the scroll box, so it counted toward scrollHeight and armed the
   scrollbar while the page still looked empty. */
const PANEL =
  'min-h-0 flex-1 overflow-y-auto overscroll-none px-(--gutter) pt-1 mb-(--dock-clear) ' +
  '[&>*]:mx-auto [&>*]:w-full [&>*]:max-w-(--measure)'

export function Settings() {
  const { activeSettingsTab, setActiveSettingsTab } = useAppStore(useShallow(s => ({
    activeSettingsTab: s.activeSettingsTab,
    setActiveSettingsTab: s.setActiveSettingsTab,
  })))
  const location = useLocation()

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
  const setTab = (v: string) => setActiveSettingsTab(v as SettingsTab)

  return (
    <Tabs value={tab} onValueChange={setTab} className="min-h-0 flex-1 overflow-hidden">
      <PageBar
        title="Settings"
        description="Configure how NexusVoice listens, formats and looks"
        nav={
          <TabsList>
            {TABS.map(({ value, label, Icon }) => (
              <TabsTrigger key={value} value={value}>
                <Icon size={12.5} strokeWidth={1.9} />
                {label}
              </TabsTrigger>
            ))}
          </TabsList>
        }
      />

      <TabsContent value={SETTINGS_TABS.GENERAL} className={PANEL}>
        <GeneralTab />
      </TabsContent>

      <TabsContent value={SETTINGS_TABS.VOICE} className={PANEL}>
        <VoiceTab />
      </TabsContent>

      <TabsContent value={SETTINGS_TABS.APPEARANCE} className={`${PANEL} flex flex-col gap-4`}>
        <AppearanceTab />
        <PillTab />
        <WaveformTab />
      </TabsContent>

      <TabsContent value={SETTINGS_TABS.ABOUT} className={PANEL}>
        <AboutTab />
      </TabsContent>
    </Tabs>
  )
}
