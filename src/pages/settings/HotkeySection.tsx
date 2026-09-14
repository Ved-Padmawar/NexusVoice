import { useState, useCallback, useRef, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { Pencil, X } from 'lucide-react'
import { COMMANDS } from '../../lib/commands'
import { extractErrorMessage } from '../../lib/errors'
import { buildShortcut, getKeyName, useRegisteredHotkeys, type HotkeyKind } from '../../lib/hotkeys'
import { useAppStore } from '../../store/useAppStore'
import { Button, IconButton } from '@/components/ui/button'
import { KeyCombo } from '@/components/ui/keycap'
import { SettingGroup, SettingRow } from '../../components/page'

type HotkeyConfig = {
  kind: HotkeyKind
  title: string
  description: string
  registerCommand: string
  unregisterCommand: string
  storeFlag: 'hasHotkey' | 'hasDictationHotkey' | 'hasDictationCommitHotkey'
}

const HOTKEY_CONFIGS: HotkeyConfig[] = [
  {
    kind: 'ptt',
    title: 'Recording hotkey',
    description: 'Hold to record. Release to transcribe and paste.',
    registerCommand: COMMANDS.REGISTER_HOTKEY,
    unregisterCommand: COMMANDS.UNREGISTER_HOTKEY,
    storeFlag: 'hasHotkey',
  },
  {
    kind: 'dictation',
    title: 'Dictation hotkey',
    description: 'Press to start, pause or resume hands-free dictation.',
    registerCommand: COMMANDS.REGISTER_DICTATION_HOTKEY,
    unregisterCommand: COMMANDS.UNREGISTER_DICTATION_HOTKEY,
    storeFlag: 'hasDictationHotkey',
  },
  {
    kind: 'dictationCommit',
    title: 'Commit dictation hotkey',
    description: 'Press to finish the current dictation without reaching for the pill.',
    registerCommand: COMMANDS.REGISTER_DICTATION_COMMIT_HOTKEY,
    unregisterCommand: COMMANDS.UNREGISTER_DICTATION_COMMIT_HOTKEY,
    storeFlag: 'hasDictationCommitHotkey',
  },
]

function HotkeyRow({ config, currentHotkey, setCurrentHotkey }: {
  config: HotkeyConfig
  currentHotkey: string | null
  setCurrentHotkey: (hotkey: string | null) => void
}) {
  const [pressedKeys, setPressedKeys] = useState<string[]>([])
  const [isListening, setIsListening] = useState(false)
  const [saving, setSaving] = useState(false)
  const rowRef = useRef<HTMLDivElement>(null)
  const keysRef = useRef<Set<string>>(new Set())
  const name = config.title.toLowerCase()

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
      if (rowRef.current && !rowRef.current.contains(e.target as Node)) {
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
    } catch (e) {
      toast.error(extractErrorMessage(e, `Failed to register ${name}.`))
    } finally {
      setSaving(false)
    }
  }

  const handleRemoveHotkey = async () => {
    try {
      await invoke(config.unregisterCommand)
      setCurrentHotkey(null)
      useAppStore.setState({ [config.storeFlag]: false })
    } catch (e) {
      toast.error(extractErrorMessage(e, `Failed to remove ${name}.`))
    }
  }

  const editing = isListening || pressedKeys.length > 0
  const shown = pressedKeys.length > 0 ? pressedKeys : !isListening && currentHotkey ? currentHotkey.split('+') : null

  return (
    <SettingRow ref={rowRef} title={config.title} description={config.description}>
        <div
          className="nv-recorder"
          data-listening={editing || undefined}
          onClick={startListening}
          onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') startListening() }}
          role="button"
          tabIndex={0}
          aria-label={`Click to record ${name}`}
        >
          {shown
            ? <KeyCombo keys={shown} live={pressedKeys.length > 0} />
            : <span className="nv-recorder__hint">{isListening ? 'Press keys…' : 'Click to set…'}</span>}
        </div>

        <div className="flex min-w-33 items-center justify-end gap-1">
          {editing ? (
            <>
              <Button size="sm" onClick={handleSaveHotkey} disabled={saving || pressedKeys.length === 0}>
                {saving ? 'Saving…' : 'Save'}
              </Button>
              <Button variant="ghost" size="sm" onClick={cancelListening}>
                Cancel
              </Button>
            </>
          ) : currentHotkey && (
            <>
              <IconButton label={`Change ${name}`} tone="accent" onClick={startListening}>
                <Pencil strokeWidth={1.9} />
              </IconButton>
              <IconButton label={`Remove ${name}`} tone="danger" onClick={handleRemoveHotkey}>
                <X strokeWidth={2} />
              </IconButton>
            </>
          )}
        </div>
    </SettingRow>
  )
}

export function HotkeySection() {
  const [currentHotkeys, setCurrentHotkeys] = useRegisteredHotkeys()

  return (
    <SettingGroup>
      {HOTKEY_CONFIGS.map(config => (
        <HotkeyRow
          key={config.kind}
          config={config}
          currentHotkey={currentHotkeys[config.kind]}
          setCurrentHotkey={(hotkey) => setCurrentHotkeys(current => ({ ...current, [config.kind]: hotkey }))}
        />
      ))}
    </SettingGroup>
  )
}
