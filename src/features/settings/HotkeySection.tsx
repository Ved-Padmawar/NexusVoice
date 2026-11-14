import { useState, useCallback, useRef, useEffect, memo } from 'react'
import { toast } from 'sonner'
import { KeyboardIcon, MicrophoneIcon, FloppyDiskIcon, XIcon, PencilSimpleIcon, RecordIcon, type Icon } from '@phosphor-icons/react'
import { commands } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { SUPER_KEY_LABEL } from '../../shared/platform'
import type { HotkeyKind } from '../../shared/types'
import { Button } from '../../shared/ui/Button'
import { Kbd } from '../../shared/ui/Kbd'
import { useBoundHotkeys, useHotkeys } from './queries'
import { caughtUp } from '../../app/sync'

type HotkeyConfig = {
  kind: HotkeyKind
  title: string
  description: string
  icon: Icon
}

const MODIFIERS: Record<string, string> = { Control: 'Ctrl', Alt: 'Alt', Shift: 'Shift', Meta: 'Super' }

/** Names the physical key (`code`), not the character it types, so Shift+1 is
 *  "Shift+1" rather than "Shift+!" and the same key works on any layout. */
function getKeyName({ key, code }: KeyboardEvent): string {
  if (MODIFIERS[key]) return MODIFIERS[key]
  if (code.startsWith('Key')) return code.slice(3)
  if (code.startsWith('Digit')) return code.slice(5)
  if (code === 'Enter') return 'Return'
  return code
}

const KEY_DISPLAY: Record<string, string> = {
  // Super is the Windows logo key on Win/Linux and Command on macOS — the
  // internal accelerator token stays "Super" everywhere; only the label differs.
  Ctrl: 'Ctrl', Super: SUPER_KEY_LABEL, Return: 'Enter', Delete: 'Del',
  ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right',
}
const displayKey = (k: string) => KEY_DISPLAY[k] ?? k

function buildShortcut(keys: string[]): string {
  const ORDER = ['Ctrl', 'Alt', 'Shift', 'Super']
  const mods: string[] = []
  let main = ''
  for (const k of keys) {
    if (['Ctrl', 'Alt', 'Shift', 'Super', 'Win', 'Cmd'].includes(k)) {
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

function HotkeyCard({ config, currentHotkey, inactive }: {
  config: HotkeyConfig
  currentHotkey: string | null
  /** Saved, but the OS did not grant it, usually because another app holds it. */
  inactive: boolean
}) {
  const [pressedKeys, setPressedKeys] = useState<string[]>([])
  const [isListening, setIsListening] = useState(false)
  const [saving, setSaving] = useState(false)
  const hotkeyRef = useRef<HTMLDivElement>(null)
  const keysRef = useRef<Set<string>>(new Set())
  const suspendedRef = useRef(false)
  const Icon = config.icon

  const resumeHotkeys = useCallback(() => {
    if (!suspendedRef.current) return
    suspendedRef.current = false
    // A shortcut another app took meanwhile shows as not active.
    commands.resumeHotkeys().catch(() => {})
  }, [])

  // Closing the page mid-edit binds the shortcuts again.
  useEffect(() => resumeHotkeys, [resumeHotkeys])

  // Released while recording, or pressing an existing combination would fire it.
  const startListening = useCallback(() => {
    if (saving) return
    if (!suspendedRef.current) {
      suspendedRef.current = true
      commands.suspendHotkeys().catch(() => {})
    }
    setIsListening(true)
    setPressedKeys([])
    keysRef.current.clear()
  }, [saving])

  const cancelListening = useCallback(() => {
    setIsListening(false)
    setPressedKeys([])
    keysRef.current.clear()
    resumeHotkeys()
  }, [resumeHotkeys])

  useEffect(() => {
    if (!isListening) return
    const onDown = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
      // Escape backs out of the edit; it is never part of a shortcut.
      if (e.key === 'Escape') { cancelListening(); return }
      const n = getKeyName(e)
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
      if (hotkeyRef.current && !hotkeyRef.current.contains(e.target as Node)) cancelListening()
    }
    window.addEventListener('keydown', onDown, true)
    window.addEventListener('keyup', onUp, true)
    document.addEventListener('mousedown', onOutside)
    return () => {
      window.removeEventListener('keydown', onDown, true)
      window.removeEventListener('keyup', onUp, true)
      document.removeEventListener('mousedown', onOutside)
    }
  }, [isListening, cancelListening])

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
    // Claimed before the await so closing the page mid-save can't restore it.
    const wasSuspended = suspendedRef.current
    suspendedRef.current = false
    setSaving(true)
    try {
      await commands.setHotkey(config.kind, shortcut)
      await caughtUp('settings')
      toast.success(`${config.title} hotkey saved`)
    } catch (e: unknown) {
      // A failed save ends the edit, so the saved shortcuts come back now.
      suspendedRef.current = wasSuspended
      resumeHotkeys()
      toast.error(extractErrorMessage(e, `Failed to register the ${config.title.toLowerCase()} hotkey.`))
    } finally {
      setPressedKeys([])
      keysRef.current.clear()
      setSaving(false)
    }
  }

  const handleRemoveHotkey = async () => {
    try {
      await commands.setHotkey(config.kind, null)
      await caughtUp('settings')
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
        {inactive ? (
          <span className="block truncate text-xs text-danger">Not active: another app is using this shortcut</span>
        ) : (
          <span className="block truncate text-xs text-muted">{config.description}</span>
        )}
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
          <span className="flex items-center gap-1.5 text-xs font-semibold text-accent-ink">
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
            <Button variant="ghost" size="sm" onClick={cancelListening} disabled={saving}>Cancel</Button>
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
              className="grid size-8 cursor-pointer place-items-center rounded-lg text-fg-2 hover:bg-accent-soft hover:text-accent-ink disabled:cursor-default disabled:opacity-50 disabled:hover:bg-transparent disabled:hover:text-fg-2"
            >
              <PencilSimpleIcon size={14} />
            </button>
            <button
              type="button"
              aria-label={`Remove ${hotkeyName}`}
              disabled={!currentHotkey}
              onClick={handleRemoveHotkey}
              className="grid size-8 cursor-pointer place-items-center rounded-lg text-fg-2 hover:bg-danger-soft hover:text-danger disabled:cursor-default disabled:opacity-50 disabled:hover:bg-transparent disabled:hover:text-fg-2"
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
  },
  {
    kind: 'dictation',
    title: 'Dictation',
    description: 'Start, pause or resume hands-free.',
    icon: MicrophoneIcon,
  },
  {
    kind: 'dictationCommit',
    title: 'Finish dictation',
    description: 'Save without the pill’s button.',
    icon: FloppyDiskIcon,
  },
]

export function HotkeySection() {
  const hotkeys = useHotkeys().data
  const bound = useBoundHotkeys().data

  return (
    <div className="flex flex-col divide-y divide-line-soft rounded-2xl bg-panel ring-1 ring-line-soft ring-inset">
      {HOTKEY_CONFIGS.map(config => {
        const saved = hotkeys?.[config.kind] ?? null
        // Until the OS state is read, assume it matches rather than flash a warning.
        const inactive = saved !== null && bound !== undefined && bound[config.kind] !== saved
        return <HotkeyCard key={config.kind} config={config} currentHotkey={saved} inactive={inactive} />
      })}
    </div>
  )
}
