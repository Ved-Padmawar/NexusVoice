import { useRef, useEffect } from 'react'
import { useLocation } from 'react-router-dom'
import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from '../lib/commands'
import { Palette, Info, Settings2, FolderOpen, Keyboard, Boxes } from 'lucide-react'
import { useAppStore } from '../store/useAppStore'
import { SETTINGS_TABS, type SettingsTab } from '../lib/routes'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { GeneralTab } from './settings/GeneralTab'
import { ModelsTab } from './settings/ModelsTab'
import { AboutTab } from './settings/AboutTab'
import { HotkeySection } from './settings/HotkeySection'
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

  // Normalize a stale persisted value ('pill' was renamed to 'shortcuts').
  const tab: SettingsTab = activeSettingsTab === ('pill' as SettingsTab) ? 'shortcuts' : activeSettingsTab
  const setTab = (v: string) => setActiveSettingsTab(v as SettingsTab)

  return (
    <div className="flex flex-col h-full overflow-hidden px-7 py-6">
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
        <span className="text-[11px] font-semibold text-(--accent) bg-(--surface) border border-(--border-soft) px-2 py-0.75 rounded-(--r-sm) shrink-0">
          v{__APP_VERSION__}
        </span>
      </div>

      <Tabs value={tab} onValueChange={setTab} className="flex flex-col flex-1 min-h-0 gap-0!">
        <div className="flex items-center justify-between mb-4 shrink-0">
          <TabsList className="w-fit!">
            <TabsTrigger value="general" className="gap-1.25! text-[12px]!">
              <Palette size={12} strokeWidth={1.75} />
              General
            </TabsTrigger>
            <TabsTrigger value="models" className="gap-1.25! text-[12px]!">
              <Boxes size={12} strokeWidth={1.75} />
              Models
            </TabsTrigger>
            <TabsTrigger value="shortcuts" className="gap-1.25! text-[12px]!">
              <Keyboard size={12} strokeWidth={1.75} />
              Shortcuts
            </TabsTrigger>
            <TabsTrigger value="about" className="gap-1.25! text-[12px]!">
              <Info size={12} strokeWidth={1.75} />
              About
            </TabsTrigger>
          </TabsList>
          {tab === 'about' && (
            <div className="flex items-center gap-1 self-start mt-0.75">
              <button
                type="button"
                className="inline-flex items-center gap-1.25 px-2.5 h-9 rounded-(--r-md) bg-(--surface) border-none text-(--fg-2) text-[12px] font-medium cursor-pointer transition-[background,color] duration-(--t-fast) hover:text-(--fg)"
                onClick={() => invoke<void>(COMMANDS.OPEN_LOGS_FOLDER)}
                title="Open logs folder"
              >
                <FolderOpen size={12} strokeWidth={1.75} />
                Logs
              </button>
            </div>
          )}
        </div>

        <TabsContent value="general" className="flex-1 overflow-y-auto min-h-0 flex flex-col gap-6 mt-0!">
          <GeneralTab />
          <div className="h-px bg-(--border-soft)" />
          <PillTab />
        </TabsContent>

        <TabsContent value="models" className="flex-1 overflow-y-auto min-h-0 flex flex-col gap-3 mt-0!">
          <ModelsTab />
        </TabsContent>

        <TabsContent value="shortcuts" className="flex-1 overflow-y-auto min-h-0 flex flex-col gap-3 mt-0!">
          <HotkeySection />
        </TabsContent>

        <TabsContent value="about" className="flex-1 overflow-y-auto min-h-0 flex flex-col gap-3 mt-0!">
          <AboutTab />
        </TabsContent>
      </Tabs>
    </div>
  )
}
