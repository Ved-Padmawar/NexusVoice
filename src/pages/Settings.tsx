import { useEffect, useState, useCallback, useRef } from 'react'
import { useLocation } from 'react-router-dom'
import { invoke } from '@tauri-apps/api/core'
import {
  Palette, Keyboard, Cpu,
  Check, AlertCircle, CheckCircle2, X,

} from 'lucide-react'
import { useAppStore, type ThemeName, type ModelId, MODEL_OPTIONS } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Alert, AlertDescription } from '@/components/ui/alert'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'

/* ── Themes ────────────────────────────────────────────────────── */
const THEMES: {
  name: ThemeName; label: string; desc: string
  bg: string; panel: string; accent: string
}[] = [
  { name: 'void',     label: 'Void',     desc: 'True dark · Violet',   bg: '#0e0d1a', panel: '#131220', accent: '#8b7cf8' },
  { name: 'obsidian', label: 'Obsidian', desc: 'Charcoal · Cyan',      bg: '#101214', panel: '#141618', accent: '#5fc8c8' },
  { name: 'nord',     label: 'Nord',     desc: 'Blue-steel · Frost',   bg: '#2e3440', panel: '#3b4252', accent: '#88c0d0' },
  { name: 'dusk',     label: 'Dusk',     desc: 'Warm dark · Rose',     bg: '#1a1110', panel: '#1f1614', accent: '#e07060' },
  { name: 'sage',     label: 'Sage',     desc: 'Warm light · Emerald', bg: '#f7f6f1', panel: '#ffffff', accent: '#3d9e6a' },
  { name: 'paper',    label: 'Paper',    desc: 'True light · Indigo',  bg: '#f8f8fc', panel: '#ffffff', accent: '#5b4de8' },
]

/* ── Hotkey helpers ────────────────────────────────────────────── */
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

const TAB_ICONS = { general: Palette, audio: Keyboard, models: Cpu }

/* ── Component ─────────────────────────────────────────────────── */
export function Settings() {
  const {
    theme, setTheme,
    modelInfo, fetchModelInfo,
    selectedModel, setSelectedModel,
    hardwareTier, fetchHardwareTier,
    error, setError,
  } = useAppStore()

  const location = useLocation()
  const initialTab = (location.state as { tab?: string } | null)?.tab ?? 'general'
  const [tab, setTab] = useState<'general' | 'audio' | 'models'>(initialTab as 'general' | 'audio' | 'models')

  const [pendingModel, setPendingModel] = useState<ModelId | null>(null)
  const [showOverrideWarn, setShowOverrideWarn] = useState(false)
  const [currentHotkey, setCurrentHotkey] = useState<string | null>(null)
  const [pressedKeys, setPressedKeys] = useState<string[]>([])
  const [isListening, setIsListening] = useState(false)
  const [saving, setSaving] = useState(false)
  const [hotkeySuccess, setHotkeySuccess] = useState(false)
  const hotkeyRef = useRef<HTMLDivElement>(null)
  const keysRef = useRef<Set<string>>(new Set())
  const tierOrder: Record<string, number> = { low: 0, mid: 1, high: 2 }

  useEffect(() => {
    fetchModelInfo(); fetchHardwareTier(); loadHotkey()
  }, [fetchModelInfo, fetchHardwareTier])

  const loadHotkey = async () => {
    try {
      const hotkeys = await invoke<string[]>('get_registered_hotkeys')
      if (hotkeys.length > 0) setCurrentHotkey(hotkeys[0])
    } catch { /* ignore */ }
  }

  const applyModel = (id: ModelId) => {
    setSelectedModel(id)
    invoke('set_model_override', { size: id.replace('whisper-', '') }).catch(() => {})
  }

  const handleModelChange = (id: ModelId) => {
    const m = MODEL_OPTIONS.find(o => o.id === id)
    if (!m) return
    if (hardwareTier && tierOrder[m.tier] > tierOrder[hardwareTier]) {
      setPendingModel(id); setShowOverrideWarn(true)
    } else applyModel(id)
  }

  const startListening = useCallback(() => {
    setIsListening(true); setPressedKeys([]); keysRef.current.clear()
    setError(null); setHotkeySuccess(false)
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
    setSaving(true); setError(null); setHotkeySuccess(false)
    try {
      await invoke('register_hotkey', { hotkey: shortcut })
      setCurrentHotkey(shortcut)
      useAppStore.setState({ hasHotkey: true })
      setPressedKeys([]); keysRef.current.clear()
      setHotkeySuccess(true); setTimeout(() => setHotkeySuccess(false), 3000)
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

  const KeyBadges = ({ keys }: { keys: string[] }) => (
    <div className="hotkey-keys">
      {keys.map((k, i) => (
        <span key={i} style={{ display: 'flex', alignItems: 'center', gap: '3px' }}>
          {i > 0 && <span className="key-sep">+</span>}
          <span className="key-badge">{displayKey(k)}</span>
        </span>
      ))}
    </div>
  )

  const TABS = ['general', 'audio', 'models'] as const

  return (
    <div className="settings-page">
      <div className="page-header" style={{ marginBottom: '16px' }}>
        <h1 className="page-title">Settings</h1>
        <p className="page-subtitle">Configure hotkeys, appearance, and model preferences.</p>
      </div>

      {/* Tab bar */}
      <div className="nv-tabs">
        {TABS.map((t) => {
          const Icon = TAB_ICONS[t]
          return (
            <button
              key={t}
              type="button"
              className={`nv-tab ${tab === t ? 'nv-tab--active' : ''}`}
              onClick={() => setTab(t)}
            >
              <Icon size={12} strokeWidth={1.75} />
              {t.charAt(0).toUpperCase() + t.slice(1)}
            </button>
          )
        })}
      </div>

      <div className="settings-scroll">

        {/* ── General: Themes ── */}
        {tab === 'general' && (
          <div className="card">
            <div className="card__header">
              <div>
                <h2 className="card__title">Appearance</h2>
                <p className="card__desc">Choose a color scheme for your workspace.</p>
              </div>
            </div>
            <div className="card__body">
              <div className="theme-grid">
                {THEMES.map((t) => {
                  const active = theme === t.name
                  return (
                    <button
                      key={t.name}
                      type="button"
                      className={`theme-card ${active ? 'theme-card--active' : ''}`}
                      onClick={() => setTheme(t.name)}
                    >
                      <div
                        className="theme-swatch"
                        style={{
                          background: `linear-gradient(135deg, ${t.bg} 0%, ${t.panel} 50%, ${t.accent} 100%)`,
                        }}
                      />
                      <div className="theme-info">
                        <div className="theme-name">{t.label}</div>
                        <div className="theme-desc">{t.desc}</div>
                      </div>
                      {active && (
                        <div className="theme-check">
                          <Check size={9} strokeWidth={3.5} />
                        </div>
                      )}
                    </button>
                  )
                })}
              </div>
            </div>
          </div>
        )}

        {/* ── Audio: Hotkey ── */}
        {tab === 'audio' && (
          <>
            {error && (
              <div className="notice notice--error">
                <AlertCircle size={13} strokeWidth={2} style={{ flexShrink: 0, color: 'var(--danger)' }} />
                <span style={{ flex: 1 }}>{error}</span>
                <button type="button" className="notice__close" onClick={() => setError(null)}>
                  <X size={13} strokeWidth={2} />
                </button>
              </div>
            )}
            {hotkeySuccess && (
              <div className="notice notice--success">
                <CheckCircle2 size={13} strokeWidth={2} style={{ flexShrink: 0, color: 'var(--success)' }} />
                <span>Hotkey registered successfully.</span>
              </div>
            )}

            <div className="card">
              <div className="card__header">
                <div>
                  <h2 className="card__title">Recording Hotkey</h2>
                  <p className="card__desc">Hold to record · release to transcribe and paste.</p>
                </div>
              </div>
              <div className="card__body" style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>

                {/* Current */}
                {currentHotkey && (
                  <div>
                    <p className="field-label" style={{ marginBottom: '6px' }}>Active hotkey</p>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                      <KeyBadges keys={currentHotkey.split('+')} />
                      <button
                        type="button"
                        className="activity-copy"
                        onClick={handleRemoveHotkey}
                        style={{ color: 'var(--muted)' }}
                        onMouseEnter={e => (e.currentTarget.style.color = 'var(--danger)')}
                        onMouseLeave={e => (e.currentTarget.style.color = 'var(--muted)')}
                      >
                        <X size={11} strokeWidth={2} />
                        Remove
                      </button>
                    </div>
                  </div>
                )}

                {/* Recorder */}
                <div>
                  <p className="field-label" style={{ marginBottom: '6px' }}>
                    {currentHotkey ? 'Change hotkey' : 'Set hotkey'}
                  </p>
                  <div className="hotkey-recorder">
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
                        <span className="hotkey-placeholder">
                          <Keyboard size={11} strokeWidth={1.75} style={{ opacity: 0.5, flexShrink: 0 }} />
                          Click to record…
                        </span>
                      )}
                      {isListening && pressedKeys.length === 0 && (
                        <span className="hotkey-placeholder" style={{ color: 'var(--accent)' }}>
                          Press keys…
                        </span>
                      )}
                      {pressedKeys.length > 0 && <KeyBadges keys={pressedKeys} />}
                    </div>

                    <Button size="sm" onClick={handleSaveHotkey} disabled={saving || pressedKeys.length === 0}>
                      {saving ? 'Saving…' : 'Save'}
                    </Button>

                    {pressedKeys.length > 0 && (
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        onClick={() => { setPressedKeys([]); keysRef.current.clear() }}
                      >
                        Clear
                      </Button>
                    )}
                  </div>
                  <p className="field-hint" style={{ marginTop: '8px' }}>
                    Recommended: Ctrl+Shift+Space · Alt+R · Ctrl+Alt+V
                  </p>
                </div>
              </div>
            </div>
          </>
        )}

        {/* ── Models ── */}
        {tab === 'models' && (
          <>
            {showOverrideWarn && pendingModel && (
              <Alert variant="destructive" style={{ marginBottom: '12px' }}>
                <AlertDescription>
                  <p style={{ marginBottom: '10px', fontSize: '12px' }}>
                    <strong>{MODEL_OPTIONS.find(m => m.id === pendingModel)?.label}</strong> requires
                    a <strong>{MODEL_OPTIONS.find(m => m.id === pendingModel)?.tier}</strong>-tier system.
                    Your hardware is <strong>{hardwareTier}</strong>. This may cause slowness or crashes.
                  </p>
                  <div style={{ display: 'flex', gap: '8px' }}>
                    <Button
                      size="sm"
                      variant="destructive"
                      onClick={() => {
                        if (pendingModel) applyModel(pendingModel)
                        setPendingModel(null); setShowOverrideWarn(false)
                      }}
                    >
                      Use anyway
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => { setPendingModel(null); setShowOverrideWarn(false) }}>
                      Cancel
                    </Button>
                  </div>
                </AlertDescription>
              </Alert>
            )}

            <div className="card" style={{ marginBottom: '12px' }}>
              <div className="card__header">
                <div>
                  <h2 className="card__title">Model Selection</h2>
                  <p className="card__desc">Choose the Whisper model for transcription.</p>
                </div>
                {hardwareTier && (
                  <Badge variant="secondary">{hardwareTier} tier</Badge>
                )}
              </div>
              <div className="card__body" style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                <Select value={selectedModel} onValueChange={(v) => handleModelChange(v as ModelId)}>
                  <SelectTrigger>
                    <SelectValue placeholder="Select model…" />
                  </SelectTrigger>
                  <SelectContent>
                    {MODEL_OPTIONS.map((m) => (
                      <SelectItem key={m.id} value={m.id}>
                        {m.label}
                        <span style={{ marginLeft: '6px', fontSize: '10px', color: 'var(--muted)', fontWeight: 500 }}>
                          {m.tier}
                        </span>
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <p className="field-hint">
                  Larger models are more accurate but slower. Downloaded automatically on first use.
                </p>
              </div>
            </div>

            {modelInfo && (
              <div className="card">
                <div className="card__header">
                  <div>
                    <h2 className="card__title">Active Model</h2>
                    <p className="card__desc">Current runtime configuration.</p>
                  </div>
                </div>
                <div className="card__body">
                  <dl className="model-info-grid">
                    <dt>Size</dt>
                    <dd><Badge variant="secondary">{modelInfo.size}</Badge></dd>
                    <dt>Selection</dt>
                    <dd style={{ fontSize: '12px' }}>{modelInfo.reason}</dd>
                    <dt>Provider</dt>
                    <dd style={{ fontSize: '12px' }}>{modelInfo.executionProvider}</dd>
                  </dl>
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  )
}
