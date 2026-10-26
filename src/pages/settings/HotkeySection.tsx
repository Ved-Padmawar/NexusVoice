import { useShallow } from 'zustand/react/shallow'
import { useState, useCallback, useRef, useEffect, memo } from 'react'
import { toast } from 'sonner'
import { KeyboardIcon, MicrophoneIcon, FloppyDiskIcon, XIcon, PencilSimpleIcon, RecordIcon, type Icon } from '@phosphor-icons/react'
import { commands } from '../../bindings'
import { extractErrorMessage } from '../../lib/errors'
import { useAppStore } from '../../store/useAppStore'
import { SUPER_KEY_LABEL } from '../../lib/platform'
import type { HotkeyBindings, HotkeyKind } from '../../types'
import { Button } from '../../components/ui/Button'
import { Kbd } from '../../components/ui/Kbd'

type HotkeyConfig = {
  kind: HotkeyKind
  title: string
  description: string
  icon: Icon
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
    <span className="inline-flex items-center gap-1">
      {keys.map((k, idx) => <Kbd key={`${k}-${idx}`}>{displayKey(k)}</Kbd>)}
    </span>
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
    if (currentHotkey) commands.setHotkey(config.kind, currentHotkey).catch(() => {})
  }, [config.kind, currentHotkey])

  // Unbound while recording, or pressing the current combination would fire it.
  const startListening = useCallback(() => {
    if (currentHotkey) commands.setHotkey(config.kind, null).catch(() => {})
    setIsListening(true)
    setPressedKeys([])
    keysRef.current.clear()
  }, [config.kind, currentHotkey])

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
      await commands.setHotkey(config.kind, shortcut)
      setCurrentHotkey(shortcut)
      useAppStore.setState({ [config.storeFlag]: true })
      setPressedKeys([])
      keysRef.current.clear()
      toast.success(`${config.title} hotkey saved`)
    } catch (e: unknown) {
      toast.error(extractErrorMessage(e, `Failed to register the ${config.title.toLowerCase()} hotkey.`))
    } finally {
      setSaving(false)
    }
  }

  const handleRemoveHotkey = async () => {
    try {
      await commands.setHotkey(config.kind, null)
      setCurrentHotkey(null)
      useAppStore.setState({ [config.storeFlag]: false })
    } catch (e: unknown) {
      toast.error(extractErrorMessage(e, `Failed to remove the ${config.title.toLowerCase()} hotkey.`))
    }
  }

  const editing = isListening || pressedKeys.length > 0

  const hotkeyName = `${config.title.toLowerCase()} hotkey`

  return (
    <div ref={hotkeyRef} className="flex items-center gap-3 px-4 py-2.5">
      <span className={`grid size-8 shrink-0 place-items-center rounded-[10px] ${currentHotkey ? 'bg-accent-soft text-accent-ink' : 'bg-field text-muted ring-1 ring-line-soft ring-inset'}`}>
        <Icon size={16} weight="duotone" />
      </span>

      <span className="min-w-0 flex-1">
        <span className="block text-sm font-semibold text-fg">{config.title}</span>
        <span className="block truncate text-xs text-muted">{config.description}</span>
      </span>

      <button
        type="button"
        onClick={startListening}
        aria-label={`Click to record ${hotkeyName}`}
        className={`flex h-9 min-w-40 cursor-pointer items-center justify-center gap-1 rounded-lg px-3 ring-1 ring-inset transition ${editing ? 'bg-accent-soft ring-accent' : 'bg-field ring-line-soft hover:ring-line'}`}
      >
        {pressedKeys.length > 0 ? (
          <KeyBadges keys={pressedKeys} />
        ) : isListening ? (
          <span className="hk-listen flex items-center gap-1.5 text-xs font-semibold text-accent-ink">
            <RecordIcon size={14} weight="fill" />Press the keys
          </span>
        ) : currentHotkey ? (
          <KeyBadges keys={currentHotkey.split('+')} />
        ) : (
          <span className="text-xs text-muted">Click to set</span>
        )}
      </button>

      <span className="flex w-36 shrink-0 justify-end gap-1">
        {editing ? (
          <>
            <Button variant="ghost" size="sm" onClick={cancelListening}>Cancel</Button>
            <Button variant="primary" size="sm" onClick={handleSaveHotkey} disabled={saving || pressedKeys.length === 0}>
              {saving ? 'Saving…' : 'Save'}
            </Button>
          </>
        ) : (
          <>
            <button
              type="button"
              aria-label={`Change ${hotkeyName}`}
              disabled={!currentHotkey}
              onClick={startListening}
              className="grid size-8 cursor-pointer place-items-center rounded-lg text-fg-2 hover:bg-accent-soft hover:text-accent-ink disabled:cursor-default disabled:opacity-30 disabled:hover:bg-transparent disabled:hover:text-fg-2"
            >
              <PencilSimpleIcon size={14} />
            </button>
            <button
              type="button"
              aria-label={`Remove ${hotkeyName}`}
              disabled={!currentHotkey}
              onClick={handleRemoveHotkey}
              className="grid size-8 cursor-pointer place-items-center rounded-lg text-fg-2 hover:bg-danger-soft hover:text-danger disabled:cursor-default disabled:opacity-30 disabled:hover:bg-transparent disabled:hover:text-fg-2"
            >
              <XIcon size={14} />
            </button>
          </>
        )}
      </span>
    </div>
  )
}

const HOTKEY_CONFIGS: HotkeyConfig[] = [
  {
    kind: 'pushToTalk',
    title: 'Recording',
    description: 'Hold to record, let go to paste.',
    icon: KeyboardIcon,
    storeFlag: 'hasHotkey',
  },
  {
    kind: 'dictation',
    title: 'Dictation',
    description: 'Start, pause or resume hands-free.',
    icon: MicrophoneIcon,
    storeFlag: 'hasDictationHotkey',
  },
  {
    kind: 'dictationCommit',
    title: 'Finish dictation',
    description: 'Save without the pill’s button.',
    icon: FloppyDiskIcon,
    storeFlag: 'hasDictationCommitHotkey',
  },
]

export function HotkeySection() {
  const { hasHotkey, hasDictationHotkey, hasDictationCommitHotkey } = useAppStore(useShallow(s => ({
    hasHotkey: s.hasHotkey,
    hasDictationHotkey: s.hasDictationHotkey,
    hasDictationCommitHotkey: s.hasDictationCommitHotkey,
  })))
  const [currentHotkeys, setCurrentHotkeys] = useState<HotkeyBindings>({
    pushToTalk: null,
    dictation: null,
    dictationCommit: null,
  })

  useEffect(() => {
    if (!hasHotkey && !hasDictationHotkey && !hasDictationCommitHotkey) return
    commands.getHotkeys().then(setCurrentHotkeys).catch(() => {})
  }, [hasHotkey, hasDictationHotkey, hasDictationCommitHotkey])

  return (
    <div className="flex flex-col divide-y divide-line-soft rounded-2xl bg-panel ring-1 ring-line-soft ring-inset">
      {HOTKEY_CONFIGS.map(config => (
        <HotkeyCard
          key={config.kind}
          config={config}
          currentHotkey={currentHotkeys[config.kind]}
          setCurrentHotkey={(hotkey) => setCurrentHotkeys(current => ({ ...current, [config.kind]: hotkey }))}
        />
      ))}
    </div>
  )
}
