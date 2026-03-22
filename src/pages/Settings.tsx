import { useRef, useEffect } from 'react'
import { useLocation } from 'react-router-dom'
import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from '../lib/commands'
import { Palette, Info, Settings2, FolderOpen } from 'lucide-react'
import { useAppStore } from '../store/useAppStore'
import { SETTINGS_TABS, type SettingsTab } from '../lib/routes'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { GeneralTab } from './settings/GeneralTab'
import { AboutTab } from './settings/AboutTab'
import { HotkeySection } from './settings/HotkeySection'
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

  const tab = activeSettingsTab
  const setTab = (v: string) => setActiveSettingsTab(v as SettingsTab)

  return (
    <div className="flex flex-col h-full overflow-hidden px-7 py-6">
      <div className="flex items-center justify-between gap-4 pb-5 mb-4 border-b border-[var(--border-soft)] flex-shrink-0">
        <div className="flex items-center gap-[14px]">
          <div className="w-9 h-9 rounded-[var(--r-lg)] bg-[var(--accent-soft)] text-[var(--accent)] flex items-center justify-center flex-shrink-0">
            <Settings2 size={18} strokeWidth={2} />
          </div>
          <div>
            <h1 className="text-[18px] font-bold tracking-[-0.025em] text-[var(--fg)] leading-[1.1] m-0">Settings</h1>
            <p className="text-[12px] text-[var(--muted)] mt-[3px] m-0">Configure hotkeys and appearance.</p>
          </div>
        </div>
        <span className="text-[11px] font-semibold text-[var(--accent)] bg-[var(--surface)] border border-[var(--border-soft)] px-[8px] py-[3px] rounded-[var(--r-sm)] flex-shrink-0">
          v{__APP_VERSION__}
        </span>
      </div>

      <Tabs value={tab} onValueChange={setTab} className="flex flex-col flex-1 min-h-0 gap-0!">
        <div className="flex items-center justify-between mb-4 flex-shrink-0">
          <TabsList className="w-fit!">
            <TabsTrigger value="general" className="gap-[5px]! text-[12px]!">
              <Palette size={12} strokeWidth={1.75} />
              General
            </TabsTrigger>
            <TabsTrigger value="about" className="gap-[5px]! text-[12px]!">
              <Info size={12} strokeWidth={1.75} />
              About
            </TabsTrigger>
          </TabsList>
          {tab === 'about' && (
            <button
              type="button"
              className="inline-flex items-center gap-[5px] px-[10px] h-9 rounded-[var(--r-lg)] bg-[var(--surface)] border-none text-[var(--fg-2)] text-[12px] font-medium cursor-pointer transition-[background,color] duration-[var(--t-fast)] hover:text-[var(--fg)]"
              onClick={() => invoke<void>(COMMANDS.OPEN_LOGS_FOLDER)}
              title="Open logs folder"
            >
              <FolderOpen size={12} strokeWidth={1.75} />
              Logs
            </button>
          )}
        </div>

        <TabsContent value="general" className="flex-1 overflow-y-auto min-h-0 flex flex-col gap-6 mt-0!">
          <GeneralTab />
          <div className="h-px bg-[var(--border-soft)]" />
          <HotkeySection />
        </TabsContent>

        <TabsContent value="about" className="flex-1 overflow-y-auto min-h-0 flex flex-col gap-3 mt-0!">
          <AboutTab />
        </TabsContent>
      </Tabs>
    </div>
  )
}
