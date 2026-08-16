import { useRef, useEffect } from 'react'
import { useLocation } from 'react-router'
import { invoke } from '@tauri-apps/api/core'
import { motion } from 'framer-motion'
import { COMMANDS } from '../lib/commands'
import { Palette, Info, Settings2, FolderOpen, Keyboard, Mic } from 'lucide-react'
import { useAppStore } from '../store/useAppStore'
import { SETTINGS_TABS, type SettingsTab } from '../lib/routes'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { AppearanceTab } from './settings/AppearanceTab'
import { AboutTab } from './settings/AboutTab'
import { HotkeySection } from './settings/HotkeySection'
import { VoiceTab } from './settings/VoiceTab'
import { PillTab } from './settings/PillTab'
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

  // Normalize stale persisted values ('pill' → 'shortcuts', 'general' → 'appearance').
  const tab: SettingsTab =
    activeSettingsTab === ('pill' as SettingsTab)
      ? 'shortcuts'
      : activeSettingsTab === ('general' as SettingsTab)
        ? 'appearance'
        : activeSettingsTab
  const setTab = (v: string) => setActiveSettingsTab(v as SettingsTab)

  return (
    <div className="flex flex-col h-full overflow-hidden px-8 pt-7 pb-4">
      <div className="flex items-center justify-between gap-4 pb-5 mb-4 border-b border-(--border-soft) shrink-0">
        <div className="flex items-center gap-3.5">
          <div className="w-9 h-9 rounded-(--r-lg) bg-(--accent-soft) text-(--accent) flex items-center justify-center shrink-0">
            <Settings2 size={18} strokeWidth={2} />
          </div>
          <div>
            <h1 className="text-[18px] font-bold tracking-tight text-(--fg) leading-[1.1] m-0">Settings</h1>
            <p className="text-[12px] text-muted-foreground mt-0.75 m-0">Configure hotkeys and appearance.</p>
          </div>
        </div>
      </div>

      <Tabs value={tab} onValueChange={setTab} className="flex flex-col flex-1 min-h-0 gap-0!">
        <div className="flex items-center justify-between mb-4 shrink-0">
          <TabsList className="w-fit!">
            <TabsTrigger value="appearance" className="gap-1.25! text-[12px]!">
              <Palette size={12} strokeWidth={1.75} />
              Appearance
            </TabsTrigger>
            <TabsTrigger value="shortcuts" className="gap-1.25! text-[12px]!">
              <Keyboard size={12} strokeWidth={1.75} />
              Shortcuts
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
          <div className="h-px bg-(--border-soft)" />
          <PillTab />
        </TabsContent>

        <TabsContent value="shortcuts" className="flex-1 overflow-y-auto overscroll-none min-h-0 flex flex-col gap-5 mt-0! pr-1">
          <HotkeySection />
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
