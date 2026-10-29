import { useShallow } from 'zustand/react/shallow'
import { useRef, useEffect } from 'react'
import { useLocation } from 'react-router'
import { InfoIcon, MicrophoneIcon, PaletteIcon, SlidersHorizontalIcon, type Icon } from '@phosphor-icons/react'
import { useAppStore } from '../../app/store/useAppStore'
import { SETTINGS_TABS, type SettingsTab } from '../../app/routes'
import { cn } from '../../shared/utils'
import { Segmented } from '../../shared/ui/Segmented'
import { AppearanceTab } from './AppearanceTab'
import { AboutTab } from './AboutTab'
import { GeneralTab } from './GeneralTab'
import { VoiceTab } from './VoiceTab'
import { PillTab } from './PillTab'
import { WaveformTab } from './WaveformTab'

const tabLabel = (Icon: Icon, text: string) => (
  <>
    <Icon size={15} weight="bold" />
    {text}
  </>
)

const TABS = [
  { value: SETTINGS_TABS.GENERAL, label: tabLabel(SlidersHorizontalIcon, 'General') },
  { value: SETTINGS_TABS.VOICE, label: tabLabel(MicrophoneIcon, 'Voice') },
  { value: SETTINGS_TABS.APPEARANCE, label: tabLabel(PaletteIcon, 'Appearance') },
  { value: SETTINGS_TABS.ABOUT, label: tabLabel(InfoIcon, 'About') },
] as const

export function Settings() {
  const { tab, setTab } = useAppStore(useShallow(s => ({
    tab: s.activeSettingsTab,
    setTab: s.setActiveSettingsTab,
  })))
  const location = useLocation()

  // Another page can open a specific tab: navigate(ROUTES.SETTINGS, { state: { tab } }).
  const initialLocationState = useRef(location.state)
  useEffect(() => {
    const requested = (initialLocationState.current as { tab?: string } | null)?.tab
    if (requested && (Object.values(SETTINGS_TABS) as string[]).includes(requested)) {
      setTab(requested as SettingsTab)
    }
  }, [setTab])

  // Voice keeps the title, tabs and filter row still; only its grid scrolls.
  const voice = tab === SETTINGS_TABS.VOICE

  return (
    <div
      className={cn(
        'min-h-0 px-7 pt-3',
        voice ? 'flex flex-col overflow-hidden [scrollbar-gutter:stable]' : 'page-scroll settings-scroll overflow-y-auto overscroll-none',
      )}
    >
      <div className="flex items-center gap-4">
        <h1 className="text-2xl font-bold tracking-[-0.03em] text-fg">Settings</h1>
        <Segmented kind="tabs" aria-label="Settings sections" value={tab} options={TABS} onChange={setTab} className="ml-auto" />
      </div>

      <div
        key={tab}
        role="tabpanel"
        className={cn('mt-5 animate-[page-in_260ms_var(--ease-out-quint)]', voice && 'flex min-h-0 flex-1 flex-col')}
      >
        {tab === SETTINGS_TABS.GENERAL && <GeneralTab />}
        {tab === SETTINGS_TABS.VOICE && <VoiceTab />}
        {tab === SETTINGS_TABS.APPEARANCE && (
          <div className="flex flex-col gap-5">
            <AppearanceTab />
            <PillTab />
            <WaveformTab />
          </div>
        )}
        {tab === SETTINGS_TABS.ABOUT && <AboutTab />}
      </div>
      {!voice && <div aria-hidden="true" className="settings-end" />}
    </div>
  )
}
