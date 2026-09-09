import { useShallow } from 'zustand/react/shallow'
import { useState, useCallback, useRef, useEffect, memo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { COMMANDS } from '../../lib/commands'
import { toast } from 'sonner'
import { Keyboard, Mic, Save, X, Pencil } from 'lucide-react'
import { useAppStore } from '../../store/useAppStore'
import { parseRegisteredHotkeys } from '../../store/modelSlice'
import { SUPER_KEY_LABEL } from '../../lib/platform'

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
  // Super is the Windows logo key on Win/Linux and Command on macOS — the
  // internal accelerator token stays "Super" everywhere; only the label differs.
  Ctrl: 'Ctrl', Super: SUPER_KEY_LABEL, Return: 'Enter',
  Backspace: 'Backspace', Delete: 'Del', Escape: 'Esc',
  ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right',
}
const displayKey = (k: string) => KEY_DISPLAY[k] ?? k

function buildShortcut(keys: string[]): string {
  const ORDER = ['Ctrl', 'Alt', 'Shift', 'Super']
  const mods: string[] = []
  let main = ''
  for (const k of keys) {
    if (['Ctrl', 'Alt', 'Shift', 'Win', 'Cmd'].includes(k)) {
      mods.push(k === 'Win' || k === 'Cmd' ? 'Super' : k)
    } else {
      main = k
    }
  }
  mods.sort((a, b) => ORDER.indexOf(a) - ORDER.indexOf(b))
  return main ? [...mods, main].join('+') : mods.join('+')
}

const KeyBadges = memo(function KeyBadges({ keys }: { keys: string[] }) {
  return (
    <span className="flex items-center gap-1">
      {keys.map((k, idx) => (
        <span key={`${k}-${idx}`} className="flex items-center gap-1">
          {idx > 0 && <span className="text-[9px] font-semibold text-(--faint)">+</span>}
          <span className="keycap">{displayKey(k)}</span>
        </span>
      ))}
    </span>
  )
})

function HotkeyRow({ config, currentHotkey, setCurrentHotkey }: {
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
      className="group flex items-center gap-3 border-b border-(--hairline) px-4 py-3 last:border-0"
    >
      <span
        className={`grid size-7 shrink-0 place-items-center rounded-(--r-sm) ${
          currentHotkey ? 'bg-(--accent-soft) text-(--on-soft)' : 'bg-(--surface) text-(--muted)'
        }`}
      >
        <Icon size={13} strokeWidth={1.9} />
      </span>

      <div className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-[12.5px] font-medium text-(--fg)">{config.title}</span>
        <span className="truncate text-[11px] text-(--muted)">{config.description}</span>
      </div>

      {/* Recorder */}
      <div
        role="button"
        tabIndex={0}
        aria-label={`Click to record ${config.title.toLowerCase()}`}
        onClick={startListening}
        onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') startListening() }}
        className={`flex h-8 min-w-34 shrink-0 cursor-pointer items-center justify-center gap-1.5 rounded-(--r-md) px-2.5 transition-[background,box-shadow] duration-(--t-fast) ${
          editing
            ? 'bg-(--accent-soft) shadow-[inset_0_0_0_1px_var(--accent-line)]'
            : 'bg-(--surface) shadow-[inset_0_0_0_1px_var(--hairline)] hover:shadow-[inset_0_0_0_1px_var(--border)]'
        }`}
      >
        {isListening && pressedKeys.length === 0 && (
          <span className="text-[11.5px] text-(--on-soft)">Press keys…</span>
        )}
        {pressedKeys.length > 0 && <KeyBadges keys={pressedKeys} />}
        {!isListening && pressedKeys.length === 0 && currentHotkey && (
          <KeyBadges keys={currentHotkey.split('+')} />
        )}
        {!isListening && pressedKeys.length === 0 && !currentHotkey && (
          <span className="text-[11.5px] text-(--muted)">Click to set…</span>
        )}
      </div>

      {/* Sized to its content, not to the widest state. A fixed width here
          had to reserve room for Save + Cancel, which left the pencil and X
          stranded ~70px from the shortcut they act on. The flexible title
          column absorbs the difference when editing starts.

          min-w-12.5 is the natural width of the pencil + X pair, so a row
          whose hotkey is unset ("Not set") still lines its recorder up with
          the rows that have one. */}
      <div className="flex min-w-12.5 shrink-0 items-center justify-end gap-1.5">
        {editing ? (
          <>
            <button
              type="button"
              onClick={handleSaveHotkey}
              disabled={saving || pressedKeys.length === 0}
              className="btn btn-sm btn-primary"
            >
              {saving ? 'Saving…' : 'Save'}
            </button>
            <button type="button" onClick={cancelListening} className="btn btn-sm btn-ghost">
              Cancel
            </button>
          </>
        ) : currentHotkey ? (
          <div className="flex items-center gap-0.5">
            <button
              type="button"
              aria-label={`Change ${config.title.toLowerCase()}`}
              title="Change"
              onClick={startListening}
              className="iconbtn iconbtn-accent"
            >
              <Pencil size={12} strokeWidth={2} />
            </button>
            <button
              type="button"
              aria-label={`Remove ${config.title.toLowerCase()}`}
              title="Remove"
              onClick={handleRemoveHotkey}
              className="iconbtn iconbtn-danger"
            >
              <X size={12} strokeWidth={2} />
            </button>
          </div>
        ) : (
          <span className="text-[10.5px] text-(--muted)">Not set</span>
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
  const { hasHotkey, hasDictationHotkey, hasDictationCommitHotkey } = useAppStore(useShallow(s => ({
    hasHotkey: s.hasHotkey,
    hasDictationHotkey: s.hasDictationHotkey,
    hasDictationCommitHotkey: s.hasDictationCommitHotkey,
  })))
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
    <>
      {HOTKEY_CONFIGS.map(config => (
        <HotkeyRow
          key={config.kind}
          config={config}
          currentHotkey={currentHotkeys[config.kind]}
          setCurrentHotkey={(hotkey) => setCurrentHotkeys(current => ({ ...current, [config.kind]: hotkey }))}
        />
      ))}
    </>
  )
}
