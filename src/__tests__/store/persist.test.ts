/**
 * The persist layer: what survives a restart, and how an old persisted blob is
 * brought forward. Both are silent when wrong — a stale pref renders nothing,
 * and a leaked field creates a second source of truth.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { STORE_PERSIST_KEY, useAppStore } from '../../store/useAppStore'
import { takeLegacyPill } from '../../store/legacyPill'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }))

const options = useAppStore.persist.getOptions()
const migrate = (blob: unknown, from: number) => options.migrate!(blob, from) as unknown as Record<string, unknown>
const partialize = options.partialize!

beforeEach(() => { takeLegacyPill() })

describe('persist — migrate', () => {
  it('hands the pill preferences older versions kept to the backend', () => {
    // The backend owns them now, so the pill window reads the same values.
    const out = migrate({ theme: 'orchid', pillTheme: 'dawn', waveformStyle: 'memo', liveTranscript: true }, 1)
    expect(takeLegacyPill()).toEqual({ theme: 'dawn', waveformStyle: 'memo', liveTranscript: true })
    expect(out).toEqual({ theme: 'orchid' })
  })

  it('moves a retired window theme to the default', () => {
    // Abyss and Pine have no stylesheet any more; kept, the window renders unthemed.
    expect(migrate({ theme: 'abyss' }, 2)).toEqual({ theme: 'indigo' })
    expect(migrate({ theme: 'pine' }, 2)).toEqual({ theme: 'indigo' })
  })

  it('rewrites the removed "steps" waveform style to "spectrum" on the way', () => {
    // `steps` has no renderer any more; the backend would reject it.
    migrate({ waveformStyle: 'steps' }, 0)
    expect(takeLegacyPill()).toEqual({ waveformStyle: 'spectrum' })
  })

  it('hands nothing over from a blob already at the current version', () => {
    migrate({ theme: 'steel' }, 3)
    expect(takeLegacyPill()).toBeNull()
  })

  it('survives an empty or partial persisted blob', () => {
    // A truncated write must not throw and block startup.
    expect(() => migrate({}, 0)).not.toThrow()
    expect(() => migrate({ theme: 'steel' }, 0)).not.toThrow()
  })
})

describe('persist — partialize', () => {
  const persisted = () => partialize(useAppStore.getState()) as Record<string, unknown>

  it('persists exactly the UI preferences', () => {
    expect(Object.keys(persisted()).sort()).toEqual([
      'activeRoute',
      'activeSettingsTab',
      'cudaOfferSeen',
      'modelChosen',
      'theme',
    ])
  })

  it('never persists the model selection', () => {
    // The backend's saved model is the single source of truth. A copy here
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
