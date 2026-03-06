import { useEffect, useState, useCallback, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore, type ThemeName, type ModelId, MODEL_OPTIONS } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'

const themeOptions: { name: ThemeName; label: string; desc: string; colors: [string, string, string] }[] = [
  { name: 'midnight', label: 'Midnight', desc: 'Dark neutral',    colors: ['#0d0c1e', '#18163a', '#7c6cef'] },
  { name: 'arctic',   label: 'Arctic',   desc: 'Light neutral',   colors: ['#f5f7fa', '#e3e8f0', '#3a6fd8'] },
  { name: 'slate',    label: 'Slate',    desc: 'Dark blue-gray',  colors: ['#16182a', '#222740', '#4fa3cc'] },
  { name: 'warm',     label: 'Warm',     desc: 'Light warm-toned',colors: ['#faf6f0', '#f0e6d4', '#c47c30'] },
]

// Returns Tauri-compatible accelerator key name (used in buildTauriShortcut)
function getKeyName(key: string, code: string): string {
  const keyMap: Record<string, string> = {
    'Control': 'Ctrl',
    'Meta': 'Super',
    ' ': 'Space',
    'ArrowUp': 'ArrowUp',
    'ArrowDown': 'ArrowDown',
    'ArrowLeft': 'ArrowLeft',
    'ArrowRight': 'ArrowRight',
    'Escape': 'Escape',
    'Delete': 'Delete',
    'Backspace': 'Backspace',
    'Enter': 'Return',
    'Tab': 'Tab',
  }
  if (keyMap[key]) return keyMap[key]
  if (key.length === 1) return key.toUpperCase()
  if (/^F\d+$/.test(key)) return key
  if (code.startsWith('Key')) return code.slice(3)
  if (code.startsWith('Digit')) return code.slice(5)
  return key
}

// Human-readable display version of a key name
const KEY_DISPLAY: Record<string, string> = {
  'Ctrl': 'Ctrl',
  'Super': 'Win',
  'Return': '↵',
  'Backspace': '⌫',
  'Delete': 'Del',
  'Escape': 'Esc',
  'ArrowUp': '↑',
  'ArrowDown': '↓',
  'ArrowLeft': '←',
  'ArrowRight': '→',
}

function displayKey(k: string): string {
  return KEY_DISPLAY[k] ?? k
}

function buildTauriShortcut(keys: string[]): string {
  const modifierOrder = ['Ctrl', 'Alt', 'Shift', 'Win']
  const modifiers: string[] = []
  let mainKey = ''
  for (const key of keys) {
    if (modifierOrder.includes(key)) {
      modifiers.push(key === 'Win' ? 'Super' : key)
    } else {
      mainKey = key
    }
  }
  modifiers.sort((a, b) => {
    const order = ['Ctrl', 'Alt', 'Shift', 'Super']
    return order.indexOf(a) - order.indexOf(b)
  })
  if (mainKey) return [...modifiers, mainKey].join('+')
  return modifiers.join('+')
}

export function Settings() {
  const {
    theme, setTheme,
    modelInfo, fetchModelInfo,
    selectedModel, setSelectedModel,
    hardwareTier, fetchHardwareTier,
    error, setError,
  } = useAppStore()

  const [pendingModel, setPendingModel] = useState<ModelId | null>(null)
  const [showOverrideWarning, setShowOverrideWarning] = useState(false)

  const [currentHotkey, setCurrentHotkey] = useState<string | null>(null)
  const [pressedKeys, setPressedKeys] = useState<string[]>([])
  const [isListening, setIsListening] = useState(false)
  const [saving, setSaving] = useState(false)
  const [hotkeySuccess, setHotkeySuccess] = useState(false)
  const hotkeyRef = useRef<HTMLDivElement>(null)
  const keysRef = useRef<Set<string>>(new Set())

  useEffect(() => {
    fetchModelInfo()
    fetchHardwareTier()
    loadCurrentHotkey()
  }, [fetchModelInfo, fetchHardwareTier])

  const tierOrder: Record<string, number> = { low: 0, mid: 1, high: 2 }

  const applyModel = (modelId: ModelId) => {
    setSelectedModel(modelId)
    // Extract size token from model id (e.g. 'whisper-small' → 'small')
    const size = modelId.replace('whisper-', '')
    invoke('set_model_override', { size }).catch(() => {})
  }

  const handleModelChange = (modelId: ModelId) => {
    const model = MODEL_OPTIONS.find(m => m.id === modelId)
    if (!model) return
    if (hardwareTier && tierOrder[model.tier] > tierOrder[hardwareTier]) {
      setPendingModel(modelId)
      setShowOverrideWarning(true)
    } else {
      applyModel(modelId)
    }
  }

  const confirmOverride = () => {
    if (!pendingModel) return
    applyModel(pendingModel)
    setPendingModel(null)
    setShowOverrideWarning(false)
  }

  const cancelOverride = () => {
    setPendingModel(null)
    setShowOverrideWarning(false)
  }

  const loadCurrentHotkey = async () => {
    try {
      const hotkeys = await invoke<string[]>('get_registered_hotkeys')
      if (hotkeys.length > 0) setCurrentHotkey(hotkeys[0])
    } catch (e) {
      console.error('Failed to load hotkey:', e)
    }
  }

  const startListening = useCallback(() => {
    setIsListening(true)
    setPressedKeys([])
    keysRef.current.clear()
    setError(null)
    setHotkeySuccess(false)
  }, [setError])

  useEffect(() => {
    if (!isListening) return

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
      const displayName = getKeyName(e.key, e.code)
      if (!keysRef.current.has(displayName)) {
        keysRef.current.add(displayName)
        setPressedKeys(Array.from(keysRef.current))
      }
    }

    const handleKeyUp = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
      setTimeout(() => setIsListening(false), 200)
    }

    const handleClickOutside = (e: MouseEvent) => {
      if (hotkeyRef.current && !hotkeyRef.current.contains(e.target as Node)) {
        setIsListening(false)
        setPressedKeys([])
        keysRef.current.clear()
      }
    }

    window.addEventListener('keydown', handleKeyDown, true)
    window.addEventListener('keyup', handleKeyUp, true)
    document.addEventListener('mousedown', handleClickOutside)

    return () => {
      window.removeEventListener('keydown', handleKeyDown, true)
      window.removeEventListener('keyup', handleKeyUp, true)
      document.removeEventListener('mousedown', handleClickOutside)
    }
  }, [isListening])

  const handleSaveHotkey = async () => {
    if (pressedKeys.length === 0) {
      setError('Press keys to record a hotkey first')
      return
    }
    const shortcut = buildTauriShortcut(pressedKeys)
    if (!shortcut) {
      setError('Invalid key combination. Use modifier + key.')
      return
    }
    setSaving(true)
    setError(null)
    setHotkeySuccess(false)
    try {
      await invoke('register_hotkey', { hotkey: shortcut })
      setCurrentHotkey(shortcut)
      setPressedKeys([])
      keysRef.current.clear()
      setHotkeySuccess(true)
      setTimeout(() => setHotkeySuccess(false), 3000)
    } catch (e: unknown) {
      const err = e as { code?: string; message?: string }
      if (err?.code === 'hotkey_already_in_use') {
        setError('This hotkey is already in use by another application.')
      } else if (err?.code === 'hotkey_permission_denied') {
        setError('OS denied hotkey registration — try a different combination.')
      } else if (err?.code === 'hotkey_invalid') {
        setError(err.message ?? 'Invalid hotkey combination.')
      } else {
        setError(err?.message ?? 'Failed to register hotkey.')
      }
    } finally {
      setSaving(false)
    }
  }

  const handleUnregisterHotkey = async () => {
    try {
      await invoke('unregister_hotkey')
      setCurrentHotkey(null)
      setHotkeySuccess(false)
    } catch (e: unknown) {
      const err = e as { message?: string }
      setError(err?.message ?? 'Failed to unregister hotkey.')
    }
  }

  const handleClearHotkey = () => {
    setPressedKeys([])
    keysRef.current.clear()
    setHotkeySuccess(false)
  }

  return (
    <div className="settings-page">
      <div className="page-header">
        <h1 className="page-title">Settings</h1>
        <p className="page-subtitle">Configure hotkeys, appearance, and model preferences.</p>
      </div>

      <Tabs defaultValue="general" className="settings-tabs">
        <TabsList className="settings-tabs-list">
          <TabsTrigger value="general">General</TabsTrigger>
          <TabsTrigger value="audio">Audio</TabsTrigger>
          <TabsTrigger value="models">Models</TabsTrigger>
        </TabsList>

        {/* ===== General Tab — Theme ===== */}
        <TabsContent value="general" className="settings-tab-content">
          <div className="card settings-section">
            <div className="card__header">
              <div>
                <h2 className="card__title">Appearance</h2>
                <p className="card__desc">Choose a color scheme that fits your workflow.</p>
              </div>
            </div>
            <div className="card__body">
              <div className="theme-grid">
                {themeOptions.map((t) => (
                  <button
                    key={t.name}
                    type="button"
                    className={`theme-card ${theme === t.name ? 'theme-card--active' : ''}`}
                    onClick={() => setTheme(t.name)}
                  >
                    <div
                      className="theme-preview"
                      style={{
                        background: `linear-gradient(135deg, ${t.colors[0]} 0%, ${t.colors[1]} 55%, ${t.colors[2]} 100%)`,
                      }}
                    />
                    <span className="theme-name">{t.label}</span>
                    <span className="theme-desc">{t.desc}</span>
                  </button>
                ))}
              </div>
            </div>
          </div>
        </TabsContent>

        {/* ===== Audio Tab — Hotkey ===== */}
        <TabsContent value="audio" className="settings-tab-content">
          {error && (
            <Alert variant="destructive" className="mb-4">
              <AlertDescription className="flex items-center justify-between">
                {error}
                <button
                  type="button"
                  onClick={() => setError(null)}
                  aria-label="Dismiss"
                  className="opacity-70 hover:opacity-100 text-base leading-none ml-2"
                >
                  ×
                </button>
              </AlertDescription>
            </Alert>
          )}

          {hotkeySuccess && (
            <Alert className="mb-4 border-green-500/30 bg-green-500/10 text-green-400">
              <AlertDescription>Hotkey registered successfully!</AlertDescription>
            </Alert>
          )}

          <div className="card settings-section">
            <div className="card__header">
              <div>
                <h2 className="card__title">Recording Hotkey</h2>
                <p className="card__desc">Press and hold to record, release to stop and transcribe.</p>
              </div>
            </div>
            <div className="card__body">
              {currentHotkey && (
                <div style={{ marginBottom: 16 }}>
                  <span className="field-label">Current hotkey</span>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 4 }}>
                    <div className="hotkey-keys">
                      {currentHotkey.split('+').map((key, i) => (
                        <span key={i}>
                          {i > 0 && <span className="key-plus">+</span>}
                          <span className="key-badge">{displayKey(key)}</span>
                        </span>
                      ))}
                    </div>
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      onClick={handleUnregisterHotkey}
                      className="text-muted-foreground hover:text-destructive"
                    >
                      Remove
                    </Button>
                  </div>
                </div>
              )}

              <div>
                <span className="field-label">New hotkey</span>
                <div className="hotkey-recorder" style={{ marginTop: 6 }}>
                  <div
                    ref={hotkeyRef}
                    className={`hotkey-display ${isListening ? 'hotkey-display--listening' : ''}`}
                    onClick={startListening}
                    onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') startListening() }}
                    role="button"
                    tabIndex={0}
                    aria-label="Click to record hotkey"
                  >
                    {pressedKeys.length === 0 && !isListening && (
                      <span className="hotkey-placeholder">Click to record hotkey…</span>
                    )}
                    {isListening && pressedKeys.length === 0 && (
                      <span className="hotkey-placeholder" style={{ color: 'var(--accent-color)' }}>
                        Press keys…
                      </span>
                    )}
                    {pressedKeys.length > 0 && (
                      <div className="hotkey-keys">
                        {pressedKeys.map((key, i) => (
                          <span key={i}>
                            {i > 0 && <span className="key-plus">+</span>}
                            <span className="key-badge">{displayKey(key)}</span>
                          </span>
                        ))}
                      </div>
                    )}
                  </div>

                  <Button
                    type="button"
                    size="sm"
                    onClick={handleSaveHotkey}
                    disabled={saving || pressedKeys.length === 0}
                  >
                    {saving ? 'Saving…' : 'Register'}
                  </Button>

                  {pressedKeys.length > 0 && (
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      onClick={handleClearHotkey}
                    >
                      Clear
                    </Button>
                  )}
                </div>
                <p className="field-hint" style={{ marginTop: 6 }}>
                  Recommended: Ctrl+Shift+Space, Alt+R, Ctrl+Alt+V
                </p>
              </div>
            </div>
          </div>
        </TabsContent>

        {/* ===== Models Tab ===== */}
        <TabsContent value="models" className="settings-tab-content">
          {/* Override warning */}
          {showOverrideWarning && pendingModel && (
            <Alert variant="destructive" className="mb-4">
              <AlertDescription>
                <p className="mb-3">
                  <strong>{MODEL_OPTIONS.find(m => m.id === pendingModel)?.label}</strong> requires a{' '}
                  <strong>{MODEL_OPTIONS.find(m => m.id === pendingModel)?.tier}</strong>-tier system.
                  Your hardware is classified as <strong>{hardwareTier}</strong>.
                  This model may cause performance issues or crashes.
                </p>
                <div className="flex gap-2">
                  <Button size="sm" variant="destructive" onClick={confirmOverride}>
                    Use anyway
                  </Button>
                  <Button size="sm" variant="ghost" onClick={cancelOverride}>
                    Cancel
                  </Button>
                </div>
              </AlertDescription>
            </Alert>
          )}

          {/* Model selector */}
          <div className="card settings-section" style={{ marginBottom: 16 }}>
            <div className="card__header">
              <div>
                <h2 className="card__title">Model Selection</h2>
                <p className="card__desc">Choose the Whisper model used for transcription.</p>
              </div>
              {hardwareTier && (
                <Badge variant="secondary">
                  Hardware: {hardwareTier} tier
                </Badge>
              )}
            </div>
            <div className="card__body">
              <div className="model-selector-row">
                <select
                  className="model-select"
                  value={selectedModel}
                  onChange={(e) => handleModelChange(e.target.value as ModelId)}
                >
                  {MODEL_OPTIONS.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.label} ({m.tier} tier)
                    </option>
                  ))}
                </select>
              </div>
              <p className="field-hint">
                Heavier models produce more accurate results but require more RAM and CPU.
              </p>
            </div>
          </div>

          {/* Active model info */}
          <div className="card settings-section">
            <div className="card__header">
              <div>
                <h2 className="card__title">Active Model Info</h2>
                <p className="card__desc">Current runtime inference configuration.</p>
              </div>
            </div>
            <div className="card__body">
              {modelInfo ? (
                <dl className="model-info-grid">
                  <dt>Size</dt>
                  <dd><Badge variant="secondary">{modelInfo.size}</Badge></dd>
                  <dt>Reason</dt>
                  <dd>{modelInfo.reason}</dd>
                  <dt>Execution provider</dt>
                  <dd>{modelInfo.executionProvider}</dd>
                </dl>
              ) : (
                <p className="field-hint">Model info unavailable. Ensure the backend is running.</p>
              )}
            </div>
          </div>
        </TabsContent>
      </Tabs>
    </div>
  )
}
