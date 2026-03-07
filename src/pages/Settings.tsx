import { useEffect, useState, useCallback, useRef } from 'react'
import { useLocation } from 'react-router-dom'
import { invoke } from '@tauri-apps/api/core'
import {
  Palette, Keyboard, Info,
  Check, AlertCircle, CheckCircle2, X,
  RefreshCw, Download, ArrowUpCircle,
} from 'lucide-react'
import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { useAppStore, type ThemeName } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'

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

const TAB_ICONS = { general: Palette, audio: Keyboard, about: Info }

/* ── Update states ─────────────────────────────────────────────── */
type UpdateStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error' | 'up-to-date'

function AboutTab() {
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>('idle')
  const [updateVersion, setUpdateVersion] = useState<string | null>(null)
  const [downloadProgress, setDownloadProgress] = useState(0)
  const [updateError, setUpdateError] = useState<string | null>(null)
  const updaterRef = useRef<Awaited<ReturnType<typeof check>> | null>(null)

  const checkForUpdate = useCallback(async () => {
    setUpdateStatus('checking')
    setUpdateError(null)
    try {
      const update = await check()
      if (update?.available) {
        updaterRef.current = update
        setUpdateVersion(update.version)
        setUpdateStatus('available')
      } else {
        setUpdateStatus('up-to-date')
      }
    } catch (e) {
      setUpdateError(e instanceof Error ? e.message : 'Update check failed')
      setUpdateStatus('error')
    }
  }, [])

  const downloadAndInstall = useCallback(async () => {
    const update = updaterRef.current
    if (!update) return
    setUpdateStatus('downloading')
    setDownloadProgress(0)
    try {
      let downloaded = 0
      let total = 0
      await update.downloadAndInstall((progress) => {
        if (progress.event === 'Started') {
          total = progress.data.contentLength ?? 0
        } else if (progress.event === 'Progress') {
          downloaded += progress.data.chunkLength
          if (total > 0) setDownloadProgress(Math.round((downloaded / total) * 100))
        } else if (progress.event === 'Finished') {
          setDownloadProgress(100)
          setUpdateStatus('ready')
        }
      })
    } catch (e) {
      setUpdateError(e instanceof Error ? e.message : 'Download failed')
      setUpdateStatus('error')
    }
  }, [])

  const INFO_ROWS = [
    { label: 'Version',  value: <Badge variant="secondary">v{__APP_VERSION__}</Badge> },
    { label: 'Model',    value: <Badge variant="secondary">Whisper Large v3 Turbo</Badge> },
    { label: 'Engine',   value: <span style={{ fontSize: '12px', color: 'var(--fg)' }}>whisper.cpp (local)</span> },
    { label: 'Language', value: <span style={{ fontSize: '12px', color: 'var(--fg)' }}>English</span> },
    { label: 'Privacy',  value: <span style={{ fontSize: '12px', color: 'var(--fg)' }}>100% on-device · no telemetry</span> },
  ]

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>

      {/* App info card */}
      <div className="card">
        <div className="card__header">
          <div>
            <h2 className="card__title">NexusVoice</h2>
            <p className="card__desc">Local-first voice-to-text for power users.</p>
          </div>
        </div>
        <div className="card__body" style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
          {INFO_ROWS.map(({ label, value }) => (
            <div key={label} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <span style={{ fontSize: '12px', color: 'var(--fg-2)' }}>{label}</span>
              {value}
            </div>
          ))}
        </div>
      </div>

      {/* Updater card */}
      <div className="card">
        <div className="card__header">
          <div>
            <h2 className="card__title">Updates</h2>
            <p className="card__desc">
              {updateStatus === 'up-to-date' && 'You are on the latest version.'}
              {updateStatus === 'available' && `Version ${updateVersion} is available.`}
              {updateStatus === 'downloading' && `Downloading update… ${downloadProgress}%`}
              {updateStatus === 'ready' && 'Update downloaded. Restart to apply.'}
              {updateStatus === 'error' && (updateError ?? 'Something went wrong.')}
              {(updateStatus === 'idle' || updateStatus === 'checking') && 'Check for the latest release.'}
            </p>
          </div>
          {updateStatus === 'up-to-date' && (
            <CheckCircle2 size={16} strokeWidth={1.75} style={{ color: 'var(--success)', flexShrink: 0 }} />
          )}
          {updateStatus === 'error' && (
            <AlertCircle size={16} strokeWidth={1.75} style={{ color: 'var(--danger)', flexShrink: 0 }} />
          )}
        </div>

        {/* Progress bar — only during download */}
        {updateStatus === 'downloading' && (
          <div style={{ padding: '0 16px 12px' }}>
            <div style={{ height: '4px', borderRadius: '999px', background: 'var(--border)', overflow: 'hidden' }}>
              <div style={{
                height: '100%',
                width: `${downloadProgress}%`,
                borderRadius: '999px',
                background: 'var(--accent)',
                transition: 'width 0.3s ease',
              }} />
            </div>
          </div>
        )}

        <div className="card__body" style={{ display: 'flex', gap: '8px' }}>
          {/* Check / re-check button */}
          {(updateStatus === 'idle' || updateStatus === 'up-to-date' || updateStatus === 'error') && (
            <Button size="sm" variant="outline" onClick={checkForUpdate}>
              <RefreshCw size={12} strokeWidth={2} />
              Check for updates
            </Button>
          )}

          {updateStatus === 'checking' && (
            <Button size="sm" variant="outline" disabled>
              <RefreshCw size={12} strokeWidth={2} style={{ animation: 'spin 1s linear infinite' }} />
              Checking…
            </Button>
          )}

          {updateStatus === 'available' && (
            <Button size="sm" onClick={downloadAndInstall}>
              <Download size={12} strokeWidth={2} />
              Download v{updateVersion}
            </Button>
          )}

          {updateStatus === 'downloading' && (
            <Button size="sm" disabled>
              <Download size={12} strokeWidth={2} />
              Downloading… {downloadProgress}%
            </Button>
          )}

          {updateStatus === 'ready' && (
            <Button size="sm" onClick={() => relaunch()}>
              <ArrowUpCircle size={12} strokeWidth={2} />
              Restart to update
            </Button>
          )}
        </div>
      </div>
    </div>
  )
}

/* ── Component ─────────────────────────────────────────────────── */
export function Settings() {
  const {
    theme, setTheme,
    error, setError,
  } = useAppStore()

  const location = useLocation()
  const initialTab = (location.state as { tab?: string } | null)?.tab ?? 'general'
  const [tab, setTab] = useState<'general' | 'audio' | 'about'>(initialTab as 'general' | 'audio' | 'about')

  const [currentHotkey, setCurrentHotkey] = useState<string | null>(null)
  const [pressedKeys, setPressedKeys] = useState<string[]>([])
  const [isListening, setIsListening] = useState(false)
  const [saving, setSaving] = useState(false)
  const [hotkeySuccess, setHotkeySuccess] = useState(false)
  const hotkeyRef = useRef<HTMLDivElement>(null)
  const keysRef = useRef<Set<string>>(new Set())

  useEffect(() => {
    loadHotkey()
  }, [])

  const loadHotkey = async () => {
    try {
      const hotkeys = await invoke<string[]>('get_registered_hotkeys')
      if (hotkeys.length > 0) setCurrentHotkey(hotkeys[0])
    } catch { /* ignore */ }
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

  const TABS = ['general', 'audio', 'about'] as const

  return (
    <div className="settings-page">
      <div className="page-header" style={{ marginBottom: '16px' }}>
        <h1 className="page-title">Settings</h1>
        <p className="page-subtitle">Configure hotkeys and appearance.</p>
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

        {/* ── About ── */}
        {tab === 'about' && <AboutTab />}
      </div>
    </div>
  )
}
