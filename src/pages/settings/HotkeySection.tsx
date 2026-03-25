import { useState, useCallback, useRef, useEffect, memo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { motion } from 'framer-motion'
import { COMMANDS } from '../../lib/commands'
import { toast } from 'sonner'
import { Keyboard, X, Pencil } from 'lucide-react'
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
    // Unregister the active hotkey so it doesn't fire while recording
    if (currentHotkey) invoke(COMMANDS.UNREGISTER_HOTKEY).catch(() => {})
    setIsListening(true); setPressedKeys([]); keysRef.current.clear()
  }, [currentHotkey])

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
        // Re-register the previous hotkey since we cancelled
        if (currentHotkey) invoke(COMMANDS.REGISTER_HOTKEY, { hotkey: currentHotkey }).catch(() => {})
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
  }, [isListening, currentHotkey])

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
    <div
      ref={hotkeyRef}
      className={`flex flex-col gap-3 p-4 rounded-[var(--r-lg)] bg-[var(--surface)] border-[1.5px] transition-[border-color] duration-[var(--t-fast)] ${isListening ? 'border-[var(--accent)]' : 'border-[var(--border)]'}`}
    >
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-[6px] text-[12px] font-semibold text-[var(--fg-2)]">
          <Keyboard size={12} strokeWidth={1.75} className="text-[var(--muted)]" />
          Recording Hotkey
        </div>
        <span className={`text-[10px] ${currentHotkey ? 'text-[var(--accent)]' : 'text-[var(--muted)]'}`}>
          {isListening ? 'Listening…' : currentHotkey ? 'Active' : 'Not set'}
        </span>
      </div>

      {/* Input area */}
      <div
        className={`flex items-center gap-[6px] px-3 h-[38px] rounded-[var(--r-md)] border cursor-pointer transition-[border-color,background] duration-[var(--t-fast)] ${isListening ? 'border-[var(--accent)] bg-[var(--accent-soft)]' : 'border-[var(--border-soft)] bg-[var(--bg-alt)]'}`}
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
          <span className="text-[11px] text-[var(--muted)] italic">Click to set a hotkey…</span>
        )}
      </div>

      {/* Actions */}
      {(isListening || pressedKeys.length > 0) ? (
        <div className="flex items-center gap-[6px]">
          <Button size="sm" onClick={handleSaveHotkey} disabled={saving || pressedKeys.length === 0}>
            {saving ? 'Saving…' : 'Save'}
          </Button>
          <Button type="button" variant="ghost" size="sm"
            onClick={() => {
              setIsListening(false); setPressedKeys([]); keysRef.current.clear()
              if (currentHotkey) invoke(COMMANDS.REGISTER_HOTKEY, { hotkey: currentHotkey }).catch(() => {})
            }}>
            Cancel
          </Button>
        </div>
      ) : currentHotkey ? (
        <div className="flex items-center gap-[6px]">
          <motion.button
            type="button"
            className="inline-flex items-center justify-center gap-[5px] h-7 px-[10px] rounded-[var(--r-md)] border border-[var(--accent)] bg-transparent cursor-pointer text-[11px] font-medium text-[var(--accent)]"
            onClick={startListening}
            whileHover={{ backgroundColor: 'var(--accent-soft)' }}
            whileTap={{ scale: 0.97 }}
            transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
          >
            <Pencil size={10} strokeWidth={2} className="flex-shrink-0" />
            Change
          </motion.button>
          <motion.button
            type="button"
            className="inline-flex items-center justify-center gap-[5px] h-7 px-[10px] rounded-[var(--r-md)] border border-[var(--danger)] bg-transparent cursor-pointer text-[11px] font-medium text-[var(--danger)]"
            onClick={handleRemoveHotkey}
            whileHover={{ backgroundColor: 'color-mix(in srgb, var(--danger) 10%, transparent)' }}
            whileTap={{ scale: 0.97 }}
            transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
          >
            <X size={10} strokeWidth={2} className="flex-shrink-0" />
            Remove
          </motion.button>
        </div>
      ) : (
        <p className="text-[11px] text-[var(--muted)] leading-[1.4]">
          Hold to record · release to transcribe and paste.
        </p>
      )}
    </div>
  )
}
