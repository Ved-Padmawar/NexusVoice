/**
 * The persist layer: what survives a restart, and how an old persisted blob is
 * brought forward. Both are silent when wrong — a stale pref renders nothing,
 * and a leaked field creates a second source of truth.
 */
import { describe, it, expect, vi } from 'vitest'
import { useAppStore } from '../../store/useAppStore'
import { STORE_PERSIST_KEY } from '../../store/persistKey'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }))

const options = useAppStore.persist.getOptions()
const migrate = options.migrate!
const partialize = options.partialize!

describe('persist — migrate', () => {
  it('rewrites the removed "steps" waveform style to "spectrum"', () => {
    // `steps` has no renderer any more, so leaving it set draws an empty canvas
    // for every user who had it selected.
    const out = migrate({ waveformStyle: 'steps' }, 0) as { waveformStyle: string }
    expect(out.waveformStyle).toBe('spectrum')
  })

  it('leaves a still-valid style alone', () => {
    for (const style of ['bars', 'memo', 'eq', 'spectrum']) {
      const out = migrate({ waveformStyle: style }, 0) as { waveformStyle: string }
      expect(out.waveformStyle, `${style} was rewritten`).toBe(style)
    }
  })

  it('does not re-run the rewrite at the current version', () => {
    // A user who deliberately has a style named "steps" in a future build must
    // not have it silently changed on every load.
    const out = migrate({ waveformStyle: 'steps' }, 1) as { waveformStyle: string }
    expect(out.waveformStyle).toBe('steps')
  })

  it('carries the rest of the persisted prefs through untouched', () => {
    const stored = { theme: 'pine', pillTheme: 'dawn', waveformStyle: 'steps', liveTranscript: true }
    const out = migrate(stored, 0) as typeof stored
    expect(out).toMatchObject({ theme: 'pine', pillTheme: 'dawn', liveTranscript: true })
  })

  it('survives an empty or partial persisted blob', () => {
    // A truncated write must not throw and block startup.
    expect(() => migrate({}, 0)).not.toThrow()
    expect(() => migrate({ theme: 'abyss' }, 0)).not.toThrow()
  })
})

describe('persist — partialize', () => {
  const persisted = () => partialize(useAppStore.getState()) as Record<string, unknown>

  it('persists exactly the UI preferences', () => {
    expect(Object.keys(persisted()).sort()).toEqual([
      'activeRoute',
      'activeSettingsTab',
      'liveTranscript',
      'modelChosen',
      'pillTheme',
      'theme',
      'waveformStyle',
    ])
  })

  it('never persists the model selection', () => {
    // The Rust `model_override` file is the single source of truth. A copy here
    // would survive a backend change and disagree with it after a restart.
    useAppStore.setState({ selectedModel: 'whisper-tiny', activeModelName: 'Whisper Tiny' })
    const keys = Object.keys(persisted())
    expect(keys).not.toContain('selectedModel')
    expect(keys).not.toContain('activeModelName')
    expect(keys).not.toContain('catalog')
  })

  it('never persists transient or in-flight state', () => {
    // Rehydrating any of these would show a stale download or a dead error
    // banner on a fresh launch.
    useAppStore.setState({
      downloads: { a: { status: 'running', progress: 50 } },
      startupError: 'disk is full',
      starting: true,
      modelReady: true,
    })
    for (const key of ['downloads', 'startupError', 'starting', 'modelReady', 'updateStatus']) {
      expect(Object.keys(persisted()), `${key} must not be persisted`).not.toContain(key)
    }
  })

  it('writes under a stable storage key', () => {
    // Changing this silently resets every user's preferences.
    expect(options.name).toBe(STORE_PERSIST_KEY)
    expect(STORE_PERSIST_KEY).toBeTruthy()
  })
})
