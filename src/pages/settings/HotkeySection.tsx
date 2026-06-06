import { useState, useCallback, useRef, useEffect, memo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { motion } from 'framer-motion'
import { COMMANDS } from '../../lib/commands'
import { toast } from 'sonner'
import { Keyboard, Mic, Save, X, Pencil } from 'lucide-react'
import { useAppStore } from '../../store/useAppStore'
import { parseRegisteredHotkeys } from '../../store/modelSlice'
import { Button } from '@/components/ui/button'

type HotkeyKind = 'ptt' | 'dictation' | 'dictationCommit'

type HotkeyConfig = {
  kind: HotkeyKind
  title: string
  description: string
  icon: typeof Keyboard
  registerCommand: string
  unregisterCommand: string
  storeFlag: 'hasHotkey' | 'hasDictationHotkey' | 'hasDictationCommitHotkey'
}

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
  Ctrl: 'Ctrl', Super: 'Win', Return: 'Enter',
  Backspace: 'Backspace', Delete: 'Del', Escape: 'Esc',
  ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right',
}
const displayKey = (k: string) => KEY_DISPLAY[k] ?? k

function buildShortcut(keys: string[]): string {
  const ORDER = ['Ctrl', 'Alt', 'Shift', 'Super']
  const mods: string[] = []
  let main = ''
  for (const k of keys) {
    if (['Ctrl', 'Alt', 'Shift', 'Win'].includes(k)) {
      mods.push(k === 'Win' ? 'Super' : k)
    } else {
      main = k
    }
  }
  mods.sort((a, b) => ORDER.indexOf(a) - ORDER.indexOf(b))
  return main ? [...mods, main].join('+') : mods.join('+')
}

const KeyBadges = memo(function KeyBadges({ keys }: { keys: string[] }) {
  return (
    <div className="flex items-center gap-[3px]">
      {keys.map((k, idx) => (
        <span key={`${k}-${idx}`} className="flex items-center gap-[3px]">
          {idx > 0 && <span className="text-[9px] text-[var(--muted)] font-semibold px-px">+</span>}
          <span className="inline-flex items-center justify-center px-[6px] py-[2px] min-w-6 rounded-[var(--r-sm)] bg-[var(--bg-alt)] border border-[var(--border)] shadow-[0_1px_0_var(--border)] text-[10px] font-semibold text-[var(--fg)] leading-[1.4] capitalize font-mono">
            {displayKey(k)}
          </span>
        </span>
      ))}
    </div>
  )
})

function HotkeyCard({ config, currentHotkey, setCurrentHotkey }: {
  config: HotkeyConfig
  currentHotkey: string | null
  setCurrentHotkey: (hotkey: string | null) => void
}) {
  const [pressedKeys, setPressedKeys] = useState<string[]>([])
  const [isListening, setIsListening] = useState(false)
  const [saving, setSaving] = useState(false)
  const hotkeyRef = useRef<HTMLDivElement>(null)
  const keysRef = useRef<Set<string>>(new Set())
  const Icon = config.icon

  const restoreCurrentHotkey = useCallback(() => {
    if (currentHotkey) invoke(config.registerCommand, { hotkey: currentHotkey }).catch(() => {})
  }, [config.registerCommand, currentHotkey])

  const startListening = useCallback(() => {
    if (currentHotkey) invoke(config.unregisterCommand).catch(() => {})
    setIsListening(true)
    setPressedKeys([])
    keysRef.current.clear()
  }, [config.unregisterCommand, currentHotkey])

  useEffect(() => {
    if (!isListening) return
    const onDown = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
      const n = getKeyName(e.key, e.code)
      if (!keysRef.current.has(n)) {
        keysRef.current.add(n)
        setPressedKeys(Array.from(keysRef.current))
      }
    }
    const onUp = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
      setTimeout(() => setIsListening(false), 200)
    }
    const onOutside = (e: MouseEvent) => {
      if (hotkeyRef.current && !hotkeyRef.current.contains(e.target as Node)) {
        setIsListening(false)
        setPressedKeys([])
        keysRef.current.clear()
        restoreCurrentHotkey()
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
  }, [isListening, restoreCurrentHotkey])

  const cancelListening = () => {
    setIsListening(false)
    setPressedKeys([])
    keysRef.current.clear()
    restoreCurrentHotkey()
  }

  const handleSaveHotkey = async () => {
    if (!pressedKeys.length) {
      toast.error('Press a key combination first')
      return
    }
    const shortcut = buildShortcut(pressedKeys)
    if (!shortcut) {
      toast.error('Invalid combination - use modifier + key')
      return
    }
    setSaving(true)
    try {
      await invoke(config.registerCommand, { hotkey: shortcut })
      setCurrentHotkey(shortcut)
      useAppStore.setState({ [config.storeFlag]: true })
      setPressedKeys([])
      keysRef.current.clear()
      toast.success(`${config.title} registered`)
    } catch (e: unknown) {
      toast.error((e as { message?: string })?.message ?? `Failed to register ${config.title.toLowerCase()}.`)
    } finally {
      setSaving(false)
    }
  }

  const handleRemoveHotkey = async () => {
    try {
      await invoke(config.unregisterCommand)
      setCurrentHotkey(null)
      useAppStore.setState({ [config.storeFlag]: false })
    } catch (e: unknown) {
      toast.error((e as { message?: string })?.message ?? `Failed to remove ${config.title.toLowerCase()}.`)
    }
  }

  const editing = isListening || pressedKeys.length > 0

  return (
    <div
      ref={hotkeyRef}
      className={`flex items-center gap-3 px-3 py-2.5 rounded-[var(--r-md)] bg-[var(--surface)] border transition-[border-color] duration-[var(--t-fast)] ${editing ? 'border-[var(--accent)]' : 'border-[var(--border-soft)]'}`}
    >
      {/* Icon */}
      <div className={`flex items-center justify-center w-7 h-7 rounded-[var(--r-sm)] shrink-0 ${currentHotkey ? 'bg-[var(--accent-soft)] text-[var(--accent)]' : 'bg-[var(--bg-alt)] text-[var(--muted)]'}`}>
        <Icon size={14} strokeWidth={1.9} />
      </div>

      {/* Label + hint */}
      <div className="flex flex-col min-w-0 flex-1">
        <span className="text-[12px] font-semibold text-[var(--fg)] leading-tight truncate">{config.title}</span>
        <span className="text-[10.5px] text-[var(--muted)] leading-tight truncate">{config.description}</span>
      </div>

      {/* Recorder / key display */}
      <div
        className={`flex items-center justify-center gap-[6px] px-2.5 h-8 min-w-[120px] rounded-[var(--r-sm)] border cursor-pointer transition-[border-color,background] duration-[var(--t-fast)] shrink-0 ${editing ? 'border-[var(--accent)] bg-[var(--accent-soft)]' : 'border-[var(--border-soft)] bg-[var(--bg-alt)]'}`}
        onClick={startListening}
        onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') startListening() }}
        role="button"
        tabIndex={0}
        aria-label={`Click to record ${config.title.toLowerCase()}`}
      >
        {isListening && pressedKeys.length === 0 && (
          <span className="text-[11px] text-[var(--accent)] italic">Press keys…</span>
        )}
        {pressedKeys.length > 0 && <KeyBadges keys={pressedKeys} />}
        {!isListening && pressedKeys.length === 0 && currentHotkey && (
          <KeyBadges keys={currentHotkey.split('+')} />
        )}
        {!isListening && pressedKeys.length === 0 && !currentHotkey && (
          <span className="text-[11px] text-[var(--muted)] italic">Click to set…</span>
        )}
      </div>

      {/* Actions */}
      <div className="flex items-center gap-[6px] shrink-0">
        {editing ? (
          <>
            <Button size="sm" onClick={handleSaveHotkey} disabled={saving || pressedKeys.length === 0}>
              {saving ? 'Saving…' : 'Save'}
            </Button>
            <Button type="button" variant="ghost" size="sm" onClick={cancelListening}>
              Cancel
            </Button>
          </>
        ) : currentHotkey ? (
          <>
            <motion.button
              type="button"
              aria-label={`Change ${config.title.toLowerCase()}`}
              title="Change"
              className="inline-flex items-center justify-center w-7 h-7 rounded-[var(--r-sm)] border border-[var(--border-soft)] bg-transparent cursor-pointer text-[var(--fg-2)]"
              onClick={startListening}
              whileHover={{ backgroundColor: 'var(--accent-soft)', borderColor: 'var(--accent)', color: 'var(--accent)' }}
              whileTap={{ scale: 0.94 }}
              transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
            >
              <Pencil size={12} strokeWidth={2} />
            </motion.button>
            <motion.button
              type="button"
              aria-label={`Remove ${config.title.toLowerCase()}`}
              title="Remove"
              className="inline-flex items-center justify-center w-7 h-7 rounded-[var(--r-sm)] border border-[var(--border-soft)] bg-transparent cursor-pointer text-[var(--fg-2)]"
              onClick={handleRemoveHotkey}
              whileHover={{ backgroundColor: 'color-mix(in srgb, var(--danger) 10%, transparent)', borderColor: 'var(--danger)', color: 'var(--danger)' }}
              whileTap={{ scale: 0.94 }}
              transition={{ type: 'spring', stiffness: 300, damping: 25, mass: 0.8 }}
            >
              <X size={12} strokeWidth={2} />
            </motion.button>
          </>
        ) : (
          <span className="text-[10px] font-semibold text-[var(--muted)] px-2 py-1 rounded-(--r-sm) bg-(--bg-alt) border border-(--border-soft)">
            Not set
          </span>
        )}
      </div>
    </div>
  )
}

const HOTKEY_CONFIGS: HotkeyConfig[] = [
  {
    kind: 'ptt',
    title: 'Recording Hotkey',
    description: 'Hold to record. Release to transcribe and paste.',
    icon: Keyboard,
    registerCommand: COMMANDS.REGISTER_HOTKEY,
    unregisterCommand: COMMANDS.UNREGISTER_HOTKEY,
    storeFlag: 'hasHotkey',
  },
  {
    kind: 'dictation',
    title: 'Dictation Hotkey',
    description: 'Press to start, pause, or resume dictation.',
    icon: Mic,
    registerCommand: COMMANDS.REGISTER_DICTATION_HOTKEY,
    unregisterCommand: COMMANDS.UNREGISTER_DICTATION_HOTKEY,
    storeFlag: 'hasDictationHotkey',
  },
  {
    kind: 'dictationCommit',
    title: 'Commit Dictation Hotkey',
    description: 'Press to save the current dictation without using the pill button.',
    icon: Save,
    registerCommand: COMMANDS.REGISTER_DICTATION_COMMIT_HOTKEY,
    unregisterCommand: COMMANDS.UNREGISTER_DICTATION_COMMIT_HOTKEY,
    storeFlag: 'hasDictationCommitHotkey',
  },
]

export function HotkeySection() {
  const { hasHotkey, hasDictationHotkey, hasDictationCommitHotkey } = useAppStore()
  const [currentHotkeys, setCurrentHotkeys] = useState<Record<HotkeyKind, string | null>>({
    ptt: null,
    dictation: null,
    dictationCommit: null,
  })

  useEffect(() => {
    if (!hasHotkey && !hasDictationHotkey && !hasDictationCommitHotkey) return
    invoke<unknown>(COMMANDS.GET_REGISTERED_HOTKEYS)
      .then(raw => {
        const parsed = parseRegisteredHotkeys(raw)
        setCurrentHotkeys({
          ptt: parsed.ptt[0] ?? null,
          dictation: parsed.dictation[0] ?? null,
          dictationCommit: parsed.dictationCommit[0] ?? null,
        })
      })
      .catch(() => {})
  }, [hasHotkey, hasDictationHotkey, hasDictationCommitHotkey])

  return (
    <div className="flex flex-col gap-4">
      <div>
        <p className="text-[12px] font-semibold text-[var(--fg-2)] tracking-[-0.01em] mb-1">Keyboard shortcuts</p>
        <p className="text-[12px] text-[var(--muted)]">Set global hotkeys for recording and hands-free dictation.</p>
      </div>
      <div className="flex flex-col gap-1.5">
        {HOTKEY_CONFIGS.map(config => (
          <HotkeyCard
            key={config.kind}
            config={config}
            currentHotkey={currentHotkeys[config.kind]}
            setCurrentHotkey={(hotkey) => setCurrentHotkeys(current => ({ ...current, [config.kind]: hotkey }))}
          />
        ))}
      </div>
    </div>
  )
}
