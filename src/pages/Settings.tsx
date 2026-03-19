import { useState, useCallback, useRef, useEffect, memo } from 'react'
import { useLocation } from 'react-router-dom'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import {
  Palette, Info, Keyboard, Settings2,
  AlertCircle, X, FolderOpen,
} from 'lucide-react'
import { useAppStore } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { GeneralTab } from './settings/GeneralTab'
import { AboutTab } from './settings/AboutTab'

/* ── Hotkey helpers ─────────────────────────────────────────────── */
function getKeyName(key: string, code: string): string {
  const map: Record<string, string> = {
    Control: 'Ctrl', Meta: 'Super', ' ': 'Space',
    ArrowUp: 'ArrowUp', ArrowDown: 'ArrowDown',
    ArrowLeft: 'ArrowLeft', ArrowRight: 'ArrowRight',
    Escape: 'Escape', Delete: 'Delete',
    Backspace: 'Backspace', Enter: 'Return', Tab: 'Tab',
  }
  if (map[key]) return map[key]
  if (key.length === 1) return key.toUpperCase()
  if (/^F\d+$/.test(key)) return key
  if (code.startsWith('Key')) return code.slice(3)
  if (code.startsWith('Digit')) return code.slice(5)
  return key
}

const KEY_DISPLAY: Record<string, string> = {
  Ctrl: 'Ctrl', Super: 'Win', Return: '↵',
  Backspace: '⌫', Delete: 'Del', Escape: 'Esc',
  ArrowUp: '↑', ArrowDown: '↓', ArrowLeft: '←', ArrowRight: '→',
}
const displayKey = (k: string) => KEY_DISPLAY[k] ?? k

function buildShortcut(keys: string[]): string {
  const ORDER = ['Ctrl', 'Alt', 'Shift', 'Super']
  const mods: string[] = []
  let main = ''
  for (const k of keys) {
    if (['Ctrl', 'Alt', 'Shift', 'Win'].includes(k)) {
      mods.push(k === 'Win' ? 'Super' : k)
    } else { main = k }
  }
  mods.sort((a, b) => ORDER.indexOf(a) - ORDER.indexOf(b))
  return main ? [...mods, main].join('+') : mods.join('+')
}

const KeyBadges = memo(function KeyBadges({ keys }: { keys: string[] }) {
  return (
    <div className="flex items-center gap-[3px]">
      {keys.map((k, i) => (
        <span key={i} className="flex items-center gap-[3px]">
          {i > 0 && <span className="text-[9px] text-[var(--muted)] font-semibold px-px">+</span>}
          <span className="inline-flex items-center justify-center px-[6px] py-[2px] min-w-6 rounded-[var(--r-sm)] bg-[var(--bg-alt)] border border-[var(--border)] shadow-[0_1px_0_var(--border)] text-[10px] font-semibold text-[var(--fg)] leading-[1.4] capitalize font-mono">
            {displayKey(k)}
          </span>
        </span>
      ))}
    </div>
  )
})

/* ── Hotkey section ─────────────────────────────────────────────── */
function HotkeySection() {
  const { error, setError, hasHotkey } = useAppStore()

  const [currentHotkey, setCurrentHotkey] = useState<string | null>(null)
  const [pressedKeys, setPressedKeys] = useState<string[]>([])
  const [isListening, setIsListening] = useState(false)
  const [saving, setSaving] = useState(false)
  const hotkeyRef = useRef<HTMLDivElement>(null)
  const keysRef = useRef<Set<string>>(new Set())

  useEffect(() => {
    if (hasHotkey) {
      invoke<string[]>('get_registered_hotkeys')
        .then(hk => { if (hk.length > 0) setCurrentHotkey(hk[0]) })
        .catch(() => {})
    }
  }, [hasHotkey])

  const startListening = useCallback(() => {
    setIsListening(true); setPressedKeys([]); keysRef.current.clear()
    setError(null)
  }, [setError])

  useEffect(() => {
    if (!isListening) return
    const onDown = (e: KeyboardEvent) => {
      e.preventDefault(); e.stopPropagation()
      const n = getKeyName(e.key, e.code)
      if (!keysRef.current.has(n)) { keysRef.current.add(n); setPressedKeys(Array.from(keysRef.current)) }
    }
    const onUp = (e: KeyboardEvent) => {
      e.preventDefault(); e.stopPropagation()
      setTimeout(() => setIsListening(false), 200)
    }
    const onOutside = (e: MouseEvent) => {
      if (hotkeyRef.current && !hotkeyRef.current.contains(e.target as Node)) {
        setIsListening(false); setPressedKeys([]); keysRef.current.clear()
      }
    }
    window.addEventListener('keydown', onDown, true)
    window.addEventListener('keyup', onUp, true)
    document.addEventListener('mousedown', onOutside)
    return () => {
      window.removeEventListener('keydown', onDown, true)
      window.removeEventListener('keyup', onUp, true)
      document.removeEventListener('mousedown', onOutside)
    }
  }, [isListening])

  const handleSaveHotkey = async () => {
    if (!pressedKeys.length) { setError('Press a key combination first'); return }
    const shortcut = buildShortcut(pressedKeys)
    if (!shortcut) { setError('Invalid combination — use modifier + key'); return }
    setSaving(true); setError(null)
    try {
      await invoke('register_hotkey', { hotkey: shortcut })
      setCurrentHotkey(shortcut)
      useAppStore.setState({ hasHotkey: true })
      setPressedKeys([]); keysRef.current.clear()
      toast.success('Hotkey registered')
    } catch (e: unknown) {
      setError((e as { message?: string })?.message ?? 'Failed to register hotkey.')
    } finally { setSaving(false) }
  }

  const handleRemoveHotkey = async () => {
    try {
      await invoke('unregister_hotkey')
      setCurrentHotkey(null)
      useAppStore.setState({ hasHotkey: false })
    } catch (e: unknown) {
      setError((e as { message?: string })?.message ?? 'Failed to remove hotkey.')
    }
  }

  return (
    <div className="flex flex-col gap-4">
      {error && (
        <div className="flex items-center gap-[10px] px-[14px] py-[10px] rounded-[var(--r-lg)] text-[12px] leading-[1.4] text-[var(--fg-2)]" style={{ background: 'var(--danger-soft)', border: '1px solid oklch(from var(--danger) l c h / 0.30)' }}>
          <AlertCircle size={13} strokeWidth={2} className="flex-shrink-0 text-[var(--danger)]" />
          <span className="flex-1">{error}</span>
          <button type="button" className="ml-auto text-[var(--muted)] bg-transparent border-none cursor-pointer px-[2px] leading-none rounded-[var(--r-xs)] flex-shrink-0 opacity-60 hover:opacity-100 transition-opacity" onClick={() => setError(null)}>
            <X size={13} strokeWidth={2} />
          </button>
        </div>
      )}
      <div>
        <p className="text-[11px] font-semibold text-[var(--fg-2)] uppercase tracking-[0.03em] mb-3">Recording Hotkey</p>
        <p className="text-[12px] text-[var(--muted)] mb-4">Hold to record · release to transcribe and paste.</p>

        <div className="flex flex-col gap-3">
          <div className="flex items-center gap-2">
            {/* Unified box — shows current hotkey, pressed keys, or placeholder */}
            <div
              ref={hotkeyRef}
              className={`flex items-center gap-[6px] px-3 h-9 rounded-[var(--r-md)] bg-[var(--surface)] border-[1.5px] cursor-pointer text-[12px] transition-[border-color,box-shadow] duration-[var(--t-fast)] max-w-[240px] flex-1 hover:border-[var(--accent)] ${isListening ? 'border-[var(--accent)] shadow-[0_0_0_3px_var(--accent-soft)] hotkey-listening' : 'border-[var(--border)]'}`}
              onClick={startListening}
              onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') startListening() }}
              role="button"
              tabIndex={0}
              aria-label="Click to record hotkey"
            >
              {isListening && pressedKeys.length === 0 && (
                <span className="text-[11px] text-[var(--accent)] italic">Press keys…</span>
              )}
              {pressedKeys.length > 0 && <KeyBadges keys={pressedKeys} />}
              {!isListening && pressedKeys.length === 0 && currentHotkey && (
                <KeyBadges keys={currentHotkey.split('+')} />
              )}
              {!isListening && pressedKeys.length === 0 && !currentHotkey && (
                <span className="inline-flex items-center gap-[5px] text-[11px] text-[var(--muted)] italic">
                  <Keyboard size={11} strokeWidth={1.75} className="opacity-50 flex-shrink-0" />
                  Click to record…
                </span>
              )}
            </div>

            {/* Button transforms based on state */}
            {isListening || pressedKeys.length > 0 ? (
              <>
                <Button size="sm" onClick={handleSaveHotkey} disabled={saving || pressedKeys.length === 0}>
                  {saving ? 'Saving…' : 'Save'}
                </Button>
                <Button type="button" variant="ghost" size="sm"
                  onClick={() => { setIsListening(false); setPressedKeys([]); keysRef.current.clear() }}>
                  Cancel
                </Button>
              </>
            ) : currentHotkey ? (
              <button
                type="button"
                className="inline-flex items-center gap-[5px] h-9 px-3 rounded-[var(--r-md)] border border-[var(--border)] bg-transparent cursor-pointer text-[11px] font-medium text-[var(--muted)] transition-[color,border-color,background] duration-[var(--t-fast)] hover:text-[var(--danger)] hover:border-[var(--danger)] hover:bg-[var(--danger-soft)]"
                onClick={handleRemoveHotkey}
              >
                <X size={11} strokeWidth={2} />
                Remove
              </button>
            ) : null}
          </div>

          <p className="text-[11px] text-[var(--muted)] leading-[1.4]">
            Recommended: Ctrl+Shift+Space · Alt+R · Ctrl+Alt+V
          </p>
        </div>
      </div>
    </div>
  )
}

/* ── Settings shell ─────────────────────────────────────────────── */
export function Settings() {
  const { activeSettingsTab, setActiveSettingsTab } = useAppStore()
  const location = useLocation()

  const initialLocationState = useRef(location.state)
  useEffect(() => {
    const requested = (initialLocationState.current as { tab?: string } | null)?.tab
    if (requested && ['general', 'about'].includes(requested)) {
      setActiveSettingsTab(requested as 'general' | 'about')
    }
  }, [setActiveSettingsTab])

  const tab = activeSettingsTab
  const setTab = (v: string) => setActiveSettingsTab(v as 'general' | 'about')

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
        <span className="text-[11px] font-semibold text-[var(--muted)] bg-[var(--surface)] border border-[var(--border-soft)] px-[8px] py-[3px] rounded-[var(--r-sm)] flex-shrink-0">
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
              onClick={() => invoke('open_logs_folder')}
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
