import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore, type ThemeName } from '../store/useAppStore'

const themeOptions: ThemeName[] = [
  'dark',
  'light',
  'ocean',
  'neon',
  'graphite',
  'solar',
  'midnight',
  'emerald',
]

export function Settings() {
  const { theme, setTheme, modelInfo, fetchModelInfo, error, setError } = useAppStore()
  const [hotkey, setHotkey] = useState('')
  const [currentHotkey, setCurrentHotkey] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    fetchModelInfo()
    loadCurrentHotkey()
  }, [fetchModelInfo])

  const loadCurrentHotkey = async () => {
    try {
      const hotkeys = await invoke<string[]>('get_registered_hotkeys')
      if (hotkeys.length > 0) {
        setCurrentHotkey(hotkeys[0])
        setHotkey(hotkeys[0])
      } else {
        setCurrentHotkey(null)
      }
    } catch (e) {
      console.error('Failed to load hotkey:', e)
    }
  }

  const handleSaveHotkey = async () => {
    if (!hotkey.trim()) {
      setError('Please enter a hotkey')
      return
    }

    setSaving(true)
    setError(null)
    try {
      await invoke('register_hotkey', { hotkey: hotkey.trim() })
      setCurrentHotkey(hotkey.trim())
      alert('Hotkey registered successfully!')
    } catch (e: any) {
      const errorMsg = e?.message || e?.toString() || 'Failed to register hotkey'
      if (errorMsg.includes('already registered')) {
        setError('This hotkey is already in use by another application. Please choose a different one.')
      } else {
        setError(errorMsg)
      }
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="settings-page">
      <h2 className="settings-title">Settings</h2>

      {error && (
        <div className="error-banner" role="alert">
          {error}
          <button
            type="button"
            className="error-dismiss"
            onClick={() => setError(null)}
            aria-label="Dismiss"
          >
            ×
          </button>
        </div>
      )}

      <section className="panel settings-section">
        <h3 className="panel-subtitle">Theme</h3>
        <div className="select-wrap">
          <select
            id="theme"
            value={theme}
            onChange={(e) => setTheme(e.target.value as ThemeName)}
          >
            {themeOptions.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </div>
      </section>

      <section className="panel settings-section">
        <h3 className="panel-subtitle">Recording Hotkey</h3>
        {currentHotkey ? (
          <p className="muted-text" style={{ marginBottom: '1rem' }}>
            Current hotkey: <strong>{currentHotkey}</strong>
          </p>
        ) : (
          <p className="muted-text" style={{ marginBottom: '1rem', color: '#f59e0b' }}>
            ⚠️ No hotkey is currently set. Please configure one below.
          </p>
        )}
        <p className="muted-text" style={{ marginBottom: '1rem' }}>
          Press and hold this key combination to start recording. Release to stop.
        </p>
        <div className="field">
          <input
            id="hotkey"
            value={hotkey}
            onChange={(e) => setHotkey(e.target.value)}
            placeholder="e.g., Ctrl+Shift+Space"
            className="input"
          />
        </div>
        <button
          type="button"
          className="btn btn-primary"
          onClick={handleSaveHotkey}
          disabled={saving}
        >
          {saving ? 'Saving...' : 'Save Hotkey'}
        </button>
        <p className="muted-text" style={{ marginTop: '0.5rem', fontSize: '0.875rem' }}>
          Examples: Ctrl+Shift+Space, Alt+R, Ctrl+Alt+V, Ctrl+Shift+A
        </p>
      </section>

      <section className="panel settings-section">
        <h3 className="panel-subtitle">Model</h3>
        {modelInfo ? (
          <dl className="model-info">
            <dt>Size</dt>
            <dd>{modelInfo.size}</dd>
            <dt>Reason</dt>
            <dd>{modelInfo.reason}</dd>
            <dt>Execution provider</dt>
            <dd>{modelInfo.executionProvider}</dd>
          </dl>
        ) : (
          <p className="muted-text">Model info unavailable.</p>
        )}
      </section>
    </div>
  )
}
