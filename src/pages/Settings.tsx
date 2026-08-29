import { useRef, useEffect } from 'react'
import { useLocation } from 'react-router'
import { invoke } from '@tauri-apps/api/core'
import { motion } from 'framer-motion'
import { COMMANDS } from '../lib/commands'
import { Palette, Info, FolderOpen, SlidersHorizontal, Mic } from 'lucide-react'
import { useAppStore } from '../store/useAppStore'
import { SETTINGS_TABS, type SettingsTab } from '../lib/routes'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { AppearanceTab } from './settings/AppearanceTab'
import { AboutTab } from './settings/AboutTab'
import { GeneralTab } from './settings/GeneralTab'
import { VoiceTab } from './settings/VoiceTab'
import { PillTab } from './settings/PillTab'

const SUBTITLE: Record<SettingsTab, string> = {
  appearance: 'Themes for the window and the recording pill',
  general: 'Input device, dictation language, formatting and hotkeys',
  voice: 'Transcription model',
  about: 'Version, updates and system information',
}

export function Settings() {
  const { activeSettingsTab, setActiveSettingsTab } = useAppStore()
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
    <div className="flex flex-col h-full overflow-hidden px-8 pt-8 pb-4">
      {/* Title and subtitle share one baseline. A screen this dense cannot
          afford an icon tile and two stacked lines just to name itself. */}
      <div className="flex shrink-0 items-baseline gap-2.5 pb-4">
        <h1 className="shrink-0 text-[16px] font-bold tracking-[-0.03em] text-(--fg) m-0">Settings</h1>
        <p className="min-w-0 flex-1 truncate text-[11px] text-muted-foreground m-0">
          {SUBTITLE[tab]}
        </p>
      </div>

      <Tabs value={tab} onValueChange={setTab} className="flex flex-col flex-1 min-h-0 gap-0!">
        <div className="flex items-center justify-between mb-3 shrink-0">
          <TabsList className="w-fit!">
            <TabsTrigger value="appearance" className="gap-1.25! text-[12px]!">
              <Palette size={12} strokeWidth={1.75} />
              Appearance
            </TabsTrigger>
            <TabsTrigger value="general" className="gap-1.25! text-[12px]!">
              <SlidersHorizontal size={12} strokeWidth={1.75} />
              General
            </TabsTrigger>
            <TabsTrigger value="voice" className="gap-1.25! text-[12px]!">
              <Mic size={12} strokeWidth={1.75} />
              Voice
            </TabsTrigger>
            <TabsTrigger value="about" className="gap-1.25! text-[12px]!">
              <Info size={12} strokeWidth={1.75} />
              About
            </TabsTrigger>
          </TabsList>
          {tab === 'about' && (
            <div className="flex items-center gap-1 self-start mt-0.75">
              <motion.button
                type="button"
                className="inline-flex items-center gap-1.25 px-2.5 h-9 rounded-(--r-md) bg-(--surface) border-none text-(--fg-2) text-[12px] font-medium cursor-pointer"
                onClick={() => invoke<void>(COMMANDS.OPEN_LOGS_FOLDER)}
                title="Open logs folder"
                whileHover={{ color: 'var(--fg)' }}
                whileTap={{ scale: 0.96 }}
                transition={{ duration: 0.15 }}
              >
                <FolderOpen size={12} strokeWidth={1.75} />
                Logs
              </motion.button>
            </div>
          )}
        </div>

        <TabsContent value="appearance" className="flex-1 overflow-y-auto overscroll-none min-h-0 flex flex-col gap-6 mt-0! pr-1">
          <AppearanceTab />
          <PillTab />
        </TabsContent>

        <TabsContent value="general" className="flex-1 overflow-y-auto overscroll-none min-h-0 flex flex-col gap-5 mt-0! pr-1">
          <GeneralTab />
        </TabsContent>

        <TabsContent value="voice" className="flex-1 overflow-y-auto overscroll-none min-h-0 flex flex-col gap-3 mt-0! pr-1">
          <VoiceTab />
        </TabsContent>

        <TabsContent value="about" className="flex-1 overflow-y-auto overscroll-none min-h-0 flex flex-col gap-3 mt-0! pr-1">
          <AboutTab />
        </TabsContent>
      </Tabs>
    </div>
  )
}
