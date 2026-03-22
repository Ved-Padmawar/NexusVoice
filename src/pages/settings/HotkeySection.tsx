import { useState, useCallback, useRef, useEffect, memo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from '../../lib/commands'
import { toast } from 'sonner'
import { Keyboard, X } from 'lucide-react'
import { useAppStore } from '../../store/useAppStore'
import { Button } from '@/components/ui/button'

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
      {keys.map((k, idx) => (
        <span key={k} className="flex items-center gap-[3px]">
          {idx > 0 && <span className="text-[9px] text-[var(--muted)] font-semibold px-px">+</span>}
          <span className="inline-flex items-center justify-center px-[6px] py-[2px] min-w-6 rounded-[var(--r-sm)] bg-[var(--bg-alt)] border border-[var(--border)] shadow-[0_1px_0_var(--border)] text-[10px] font-semibold text-[var(--fg)] leading-[1.4] capitalize font-mono">
            {displayKey(k)}
          </span>
        </span>
      ))}
    </div>
  )
})

export function HotkeySection() {
  const { hasHotkey } = useAppStore()

  const [currentHotkey, setCurrentHotkey] = useState<string | null>(null)
  const [pressedKeys, setPressedKeys] = useState<string[]>([])
  const [isListening, setIsListening] = useState(false)
  const [saving, setSaving] = useState(false)
  const hotkeyRef = useRef<HTMLDivElement>(null)
  const keysRef = useRef<Set<string>>(new Set())

  useEffect(() => {
    if (hasHotkey) {
      invoke<string[]>(COMMANDS.GET_REGISTERED_HOTKEYS)
        .then(hk => { if (hk.length > 0) setCurrentHotkey(hk[0]) })
        .catch(() => {})
    }
  }, [hasHotkey])

  const startListening = useCallback(() => {
    setIsListening(true); setPressedKeys([]); keysRef.current.clear()
  }, [])

  useEffect(() => {
    if (!isListening) return
    const onDown = (e: KeyboardEvent) => {
      e.preventDefault(); e.stopPropagation()
      const n = getKeyName(e.key, e.code)
      if (!keysRef.current.has(n)) {
        keysRef.current.add(n)
        setPressedKeys(Array.from(keysRef.current))
      }
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
    if (!pressedKeys.length) { toast.error('Press a key combination first'); return }
    const shortcut = buildShortcut(pressedKeys)
    if (!shortcut) { toast.error('Invalid combination — use modifier + key'); return }
    setSaving(true)
    try {
      await invoke(COMMANDS.REGISTER_HOTKEY, { hotkey: shortcut })
      setCurrentHotkey(shortcut)
      useAppStore.setState({ hasHotkey: true })
      setPressedKeys([]); keysRef.current.clear()
      toast.success('Hotkey registered')
    } catch (e: unknown) {
      toast.error((e as { message?: string })?.message ?? 'Failed to register hotkey.')
    } finally { setSaving(false) }
  }

  const handleRemoveHotkey = async () => {
    try {
      await invoke(COMMANDS.UNREGISTER_HOTKEY)
      setCurrentHotkey(null)
      useAppStore.setState({ hasHotkey: false })
    } catch (e: unknown) {
      toast.error((e as { message?: string })?.message ?? 'Failed to remove hotkey.')
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <div>
        <p className="text-[11px] font-semibold text-[var(--fg-2)] uppercase tracking-[0.03em] mb-3">Recording Hotkey</p>
        <p className="text-[12px] text-[var(--muted)] mb-4">Hold to record · release to transcribe and paste.</p>

        <div className="flex flex-col gap-3">
          <div className="flex items-center gap-2">
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
