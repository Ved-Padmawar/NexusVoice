import { useState, useCallback, useRef, useEffect, memo } from 'react'
import { useLocation } from 'react-router-dom'
import { invoke } from '@tauri-apps/api/core'
import {
  Palette, Keyboard, Info,
  Check, AlertCircle, CheckCircle2, X,
  RefreshCw, Download, ArrowUpCircle, Cpu,
} from 'lucide-react'
import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { useAppStore, type ThemeName } from '../store/useAppStore'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'

/* ── Themes ────────────────────────────────────────────────────── */
const THEMES: {
  name: ThemeName; label: string; mode: 'dark' | 'light'
  bg: string; panel: string; accent: string; border: string; surface: string; muted: string
}[] = [
  { name: 'abyss',    label: 'Abyss',    mode: 'dark',  bg: '#22232b', panel: '#2a2b35', accent: '#78a2f4', border: '#383a48', surface: '#2e303d', muted: '#6a6e88' },
  { name: 'midnight', label: 'Midnight', mode: 'dark',  bg: '#0d101c', panel: '#131526', accent: '#8b5cf6', border: '#1e2240', surface: '#181b2e', muted: '#4a507a' },
  { name: 'nebula',   label: 'Nebula',   mode: 'dark',  bg: '#231b2a', panel: '#2c2433', accent: '#9d38a8', border: '#3e3048', surface: '#332840', muted: '#6a5878' },
  { name: 'pine',     label: 'Pine',     mode: 'dark',  bg: '#1b2420', panel: '#222d29', accent: '#58c596', border: '#304038', surface: '#283530', muted: '#507060' },
  { name: 'canvas',   label: 'Canvas',   mode: 'light', bg: '#f8f9fc', panel: '#ffffff', accent: '#3a5bd9', border: '#d8dce8', surface: '#f0f2f8', muted: '#8890b0' },
  { name: 'dawn',     label: 'Dawn',     mode: 'light', bg: '#faf4ee', panel: '#ede0d0', accent: '#d4610a', border: '#d8c8b4', surface: '#f5ede2', muted: '#9a8870' },
  { name: 'breeze',   label: 'Breeze',   mode: 'light', bg: '#eef6f8', panel: '#d8eef0', accent: '#1a7a8a', border: '#c0d8dc', surface: '#e8f4f6', muted: '#6a9098' },
  { name: 'blossom',  label: 'Blossom',  mode: 'light', bg: '#f8eef0', panel: '#e8d4d8', accent: '#c0304a', border: '#d8c0c4', surface: '#f2e4e8', muted: '#9a7078' },
]

/* Mini UI preview — renders a fake app layout using theme raw colors */
function ThemePreview({ bg, panel, accent, border, surface, muted }: {
  bg: string; panel: string; accent: string; border: string; surface: string; muted: string
}) {
  return (
    <svg viewBox="0 0 120 72" xmlns="http://www.w3.org/2000/svg" className="theme-preview">
      {/* App background */}
      <rect width="120" height="72" fill={bg} />
      {/* Titlebar */}
      <rect x="0" y="0" width="120" height="9" fill={panel} />
      <circle cx="6" cy="4.5" r="1.8" fill={muted} opacity="0.6" />
      <circle cx="11" cy="4.5" r="1.8" fill={muted} opacity="0.6" />
      <circle cx="16" cy="4.5" r="1.8" fill={muted} opacity="0.6" />
      <rect x="42" y="3" width="36" height="3" rx="1.5" fill={border} opacity="0.7" />
      {/* Sidebar */}
      <rect x="0" y="9" width="28" height="63" fill={panel} />
      <rect x="0" y="9" width="28" height="63" fill="none" stroke={border} strokeWidth="0.5" />
      {/* Sidebar nav items */}
      <rect x="4" y="16" width="3" height="3" rx="1" fill={accent} opacity="0.9" />
      <rect x="10" y="17" width="14" height="2" rx="1" fill={accent} opacity="0.5" />
      <rect x="4" y="24" width="3" height="3" rx="1" fill={muted} opacity="0.5" />
      <rect x="10" y="25" width="12" height="2" rx="1" fill={muted} opacity="0.3" />
      <rect x="4" y="32" width="3" height="3" rx="1" fill={muted} opacity="0.5" />
      <rect x="10" y="33" width="10" height="2" rx="1" fill={muted} opacity="0.3" />
      {/* Main content */}
      <rect x="32" y="14" width="22" height="3" rx="1.5" fill={muted} opacity="0.5" />
      {/* Stat cards */}
      <rect x="32" y="22" width="20" height="12" rx="2" fill={surface} stroke={border} strokeWidth="0.5" />
      <rect x="55" y="22" width="20" height="12" rx="2" fill={surface} stroke={border} strokeWidth="0.5" />
      <rect x="78" y="22" width="20" height="12" rx="2" fill={surface} stroke={border} strokeWidth="0.5" />
      <rect x="101" y="22" width="15" height="12" rx="2" fill={surface} stroke={border} strokeWidth="0.5" />
      {/* Accent bars in cards */}
      <rect x="35" y="28" width="14" height="3" rx="1" fill={accent} opacity="0.85" />
      <rect x="58" y="28" width="14" height="3" rx="1" fill={accent} opacity="0.6" />
      <rect x="81" y="28" width="14" height="3" rx="1" fill={accent} opacity="0.4" />
      <rect x="104" y="28" width="9" height="3" rx="1" fill={accent} opacity="0.25" />
      {/* Activity card */}
      <rect x="32" y="38" width="84" height="28" rx="2" fill={surface} stroke={border} strokeWidth="0.5" />
      <rect x="36" y="43" width="40" height="2" rx="1" fill={muted} opacity="0.4" />
      <rect x="36" y="48" width="60" height="2" rx="1" fill={muted} opacity="0.25" />
      <rect x="36" y="53" width="50" height="2" rx="1" fill={muted} opacity="0.2" />
      <rect x="36" y="58" width="30" height="2" rx="1" fill={accent} opacity="0.35" />
    </svg>
  )
}

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

/* ── Model selection ────────────────────────────────────────────── */
type HardwareProfile = {
  gpuName: string
  executionProvider: string
  vramGb: number
  recommendedModel: string
}

type ModelOverride = 'auto' | 'large' | 'medium'

const MODEL_OPTIONS: { value: ModelOverride; label: string; sub: string }[] = [
  { value: 'auto',   label: 'Auto',   sub: 'Recommended' },
  { value: 'large',  label: 'Large',  sub: 'Best accuracy' },
  { value: 'medium', label: 'Medium', sub: 'Faster / CPU' },
]

/* ── Update states ─────────────────────────────────────────────── */
type UpdateStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error' | 'up-to-date'

function AboutTab() {
  /* model state */
  const [profile, setProfile] = useState<HardwareProfile | null>(null)
  const [selected, setSelected] = useState<ModelOverride>('auto')
  const [modelSaving, setModelSaving] = useState(false)
  const [modelSaved, setModelSaved] = useState(false)

  /* update state */
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>('idle')
  const [updateVersion, setUpdateVersion] = useState<string | null>(null)
  const [downloadProgress, setDownloadProgress] = useState(0)
  const [updateError, setUpdateError] = useState<string | null>(null)
  const updaterRef = useRef<Awaited<ReturnType<typeof check>> | null>(null)

  useEffect(() => {
    invoke<HardwareProfile>('get_hardware_profile').then(setProfile).catch(() => {})
  }, [])

  const handleModelChange = async (v: ModelOverride) => {
    setSelected(v)
    setModelSaving(true)
    setModelSaved(false)
    try {
      if (v === 'auto') await invoke('clear_model_override')
      else await invoke('set_model_override', { variant: v })
      invoke('retry_model_download').catch(() => {})
      setModelSaved(true)
      setTimeout(() => setModelSaved(false), 2000)
    } catch { /* ignore */ }
    finally { setModelSaving(false) }
  }

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

  const updateDesc = {
    idle: 'Check for the latest release.',
    checking: 'Checking…',
    'up-to-date': 'You are on the latest version.',
    available: `v${updateVersion} is available.`,
    downloading: `Downloading… ${downloadProgress}%`,
    ready: 'Update downloaded. Restart to apply.',
    error: updateError ?? 'Something went wrong.',
  }[updateStatus]

  return (
    <div className="about-stack">

      {/* ── Single unified card ── */}
      <div className="card">

        {/* App info */}
        <div className="card__header">
          <div>
            <h2 className="card__title">NexusVoice</h2>
            <p className="card__desc">Local-first voice-to-text for power users.</p>
          </div>
          <Badge variant="secondary">v{__APP_VERSION__}</Badge>
        </div>
        <div className="card__body about-info-grid">
          <span className="about-info-label">Engine</span>
          <Badge variant="secondary">whisper-rs (ggml)</Badge>
          <span className="about-info-label">Language</span>
          <span className="about-info-value">English</span>
          <span className="about-info-label">Privacy</span>
          <span className="about-info-value">100% on-device · no telemetry</span>
        </div>

        <div className="card__divider" />

        {/* Hardware + model */}
        <div className="card__header card__header--section">
          <div>
            <h2 className="card__title">Model</h2>
            <p className="card__desc">
              {profile
                ? <><Cpu size={11} strokeWidth={1.75} className="icon--inline" />{profile.gpuName}</>
                : 'Detecting hardware…'}
            </p>
          </div>
          <div className="about-badges">
            {profile && <Badge variant="secondary">{profile.executionProvider.toUpperCase()}</Badge>}
            {profile && profile.vramGb > 0 && <Badge variant="secondary">{profile.vramGb} GB VRAM</Badge>}
            {modelSaved && <CheckCircle2 size={14} strokeWidth={2} className="icon--success" />}
          </div>
        </div>
        <div className="card__body">
          <div className="model-segment">
            {MODEL_OPTIONS.map(({ value, label, sub }) => (
              <button
                key={value}
                type="button"
                className={`model-segment__btn${selected === value ? ' model-segment__btn--active' : ''}`}
                onClick={() => handleModelChange(value)}
                disabled={modelSaving}
              >
                <span className="model-segment__label">{label}</span>
                <span className="model-segment__sub">{sub}</span>
              </button>
            ))}
          </div>
        </div>

        <div className="card__divider" />

        {/* Updater */}
        <div className="card__header card__header--section">
          <div className="about-update-left">
            <h2 className="card__title">Updates</h2>
            <p className="card__desc">{updateDesc}</p>
            {/* Progress bar — always present, width animates */}
            <div className="about-progress-track">
              <div
                className="about-progress-fill"
                style={{ width: updateStatus === 'downloading' ? `${downloadProgress}%` : '0%' }}
              />
            </div>
          </div>
          <div className="about-update-actions">
            {updateStatus === 'up-to-date' && <CheckCircle2 size={14} strokeWidth={2} className="icon--success" />}
            {updateStatus === 'error' && <AlertCircle size={14} strokeWidth={2} className="icon--danger" />}
            {(updateStatus === 'idle' || updateStatus === 'up-to-date' || updateStatus === 'error') && (
              <Button size="sm" variant="outline" onClick={checkForUpdate}>
                <RefreshCw size={11} strokeWidth={2} />
                Check
              </Button>
            )}
            {updateStatus === 'checking' && (
              <Button size="sm" variant="outline" disabled>
                <RefreshCw size={11} strokeWidth={2} className="icon--spin" />
                Checking…
              </Button>
            )}
            {updateStatus === 'available' && (
              <Button size="sm" onClick={downloadAndInstall}>
                <Download size={11} strokeWidth={2} />
                Download
              </Button>
            )}
            {updateStatus === 'downloading' && (
              <Button size="sm" disabled>
                <Download size={11} strokeWidth={2} />
                {downloadProgress}%
              </Button>
            )}
            {updateStatus === 'ready' && (
              <Button size="sm" onClick={() => relaunch()}>
                <ArrowUpCircle size={11} strokeWidth={2} />
                Restart
              </Button>
            )}
          </div>
        </div>
        {/* Spacer so card__body padding applies at bottom */}
        <div className="card__body card__body--flush-top" />
      </div>
    </div>
  )
}

/* ── Component ─────────────────────────────────────────────────── */
export function Settings() {
  const {
    theme, setTheme,
    error, setError,
    hasHotkey,
    activeSettingsTab, setActiveSettingsTab,
  } = useAppStore()

  const location = useLocation()

  // If navigated here with a specific tab (e.g. from hotkey banner), honour it once
  useEffect(() => {
    const requested = (location.state as { tab?: string } | null)?.tab
    if (requested && ['general', 'audio', 'about'].includes(requested)) {
      setActiveSettingsTab(requested as 'general' | 'audio' | 'about')
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const tab = activeSettingsTab
  const setTab = setActiveSettingsTab

  // Load initial hotkey from store — if set, fetch the string value once
  const [currentHotkey, setCurrentHotkey] = useState<string | null>(null)
  const [pressedKeys, setPressedKeys] = useState<string[]>([])
  const [isListening, setIsListening] = useState(false)
  const [saving, setSaving] = useState(false)
  const [hotkeySuccess, setHotkeySuccess] = useState(false)
  const hotkeyRef = useRef<HTMLDivElement>(null)
  const keysRef = useRef<Set<string>>(new Set())

  // Sync currentHotkey from store on mount — single source of truth
  useEffect(() => {
    if (hasHotkey) {
      invoke<string[]>('get_registered_hotkeys')
        .then(hk => { if (hk.length > 0) setCurrentHotkey(hk[0]) })
        .catch(() => {})
    }
  }, [hasHotkey])

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

  const KeyBadges = memo(({ keys }: { keys: string[] }) => (
    <div className="hotkey-keys">
      {keys.map((k, i) => (
        <span key={i} className="hotkey-key-item">
          {i > 0 && <span className="key-sep">+</span>}
          <span className="key-badge">{displayKey(k)}</span>
        </span>
      ))}
    </div>
  ))

  const TABS = (['general', 'audio', 'about'] as const)

  return (
    <div className="settings-page">
      <div className="page-header">
        <h1 className="page-title">Settings</h1>
        <p className="page-subtitle">Configure hotkeys and appearance.</p>
      </div>

      <Tabs value={tab} onValueChange={(v) => setTab(v as typeof tab)} className="settings-tabs">
        <TabsList className="settings-tabs__list">
          {TABS.map((t) => {
            const Icon = TAB_ICONS[t]
            return (
              <TabsTrigger key={t} value={t} className="settings-tabs__trigger">
                <Icon size={12} strokeWidth={1.75} />
                {t.charAt(0).toUpperCase() + t.slice(1)}
              </TabsTrigger>
            )
          })}
        </TabsList>

        {/* ── General: Themes ── */}
        <TabsContent value="general" className="settings-scroll">
          <div className="card">
            <div className="card__header">
              <div>
                <h2 className="card__title">Appearance</h2>
                <p className="card__desc">Choose a color scheme for your workspace.</p>
              </div>
            </div>
            <div className="card__body">
              {(['dark', 'light'] as const).map((mode) => {
                const group = THEMES.filter(t => t.mode === mode)
                return (
                  <div key={mode} className="theme-group">
                    <p className="theme-group__label">{mode === 'dark' ? 'Dark' : 'Light'}</p>
                    <div className="theme-grid">
                      {group.map((t) => {
                        const active = theme === t.name
                        return (
                          <button
                            key={t.name}
                            type="button"
                            className={`theme-card ${active ? 'theme-card--active' : ''}`}
                            onClick={() => setTheme(t.name)}
                          >
                            <ThemePreview bg={t.bg} panel={t.panel} accent={t.accent} border={t.border} surface={t.surface} muted={t.muted} />
                            <div className="theme-card__footer">
                              <span className="theme-name">{t.label}</span>
                              {active && <Check size={9} strokeWidth={3.5} className="theme-card__check" />}
                            </div>
                          </button>
                        )
                      })}
                    </div>
                  </div>
                )
              })}
            </div>
          </div>
        </TabsContent>

        {/* ── Audio: Hotkey ── */}
        <TabsContent value="audio" className="settings-scroll">
          {error && (
            <div className="notice notice--error">
              <AlertCircle size={13} strokeWidth={2} className="icon--shrink icon--danger" />
              <span className="text--flex">{error}</span>
              <button type="button" className="notice__close" onClick={() => setError(null)}>
                <X size={13} strokeWidth={2} />
              </button>
            </div>
          )}
          {hotkeySuccess && (
            <div className="notice notice--success">
              <CheckCircle2 size={13} strokeWidth={2} className="icon--shrink icon--success" />
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
            <div className="card__body card__body--stack">

              {currentHotkey && (
                <div>
                  <p className="field-label field-label--mb">Active hotkey</p>
                  <div className="hotkey-active-row">
                    <KeyBadges keys={currentHotkey.split('+')} />
                    <button type="button" className="hotkey-remove" onClick={handleRemoveHotkey}>
                      <X size={11} strokeWidth={2} />
                      Remove
                    </button>
                  </div>
                </div>
              )}

              <div>
                <p className="field-label field-label--mb">
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
                        <Keyboard size={11} strokeWidth={1.75} className="hotkey-placeholder__icon" />
                        Click to record…
                      </span>
                    )}
                    {isListening && pressedKeys.length === 0 && (
                      <span className="hotkey-placeholder hotkey-placeholder--listening">
                        Press keys…
                      </span>
                    )}
                    {pressedKeys.length > 0 && <KeyBadges keys={pressedKeys} />}
                  </div>

                  <Button size="sm" onClick={handleSaveHotkey} disabled={saving || pressedKeys.length === 0}>
                    {saving ? 'Saving…' : 'Save'}
                  </Button>

                  {pressedKeys.length > 0 && (
                    <Button type="button" variant="ghost" size="sm"
                      onClick={() => { setPressedKeys([]); keysRef.current.clear() }}>
                      Clear
                    </Button>
                  )}
                </div>
                <p className="field-hint field-hint--mt">
                  Recommended: Ctrl+Shift+Space · Alt+R · Ctrl+Alt+V
                </p>
              </div>
            </div>
          </div>
        </TabsContent>

        {/* ── About ── */}
        <TabsContent value="about" className="settings-scroll">
          <AboutTab />
        </TabsContent>
      </Tabs>
    </div>
  )
}
