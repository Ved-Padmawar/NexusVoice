import type { ReactElement } from 'react'
import { QueryClientProvider } from '@tanstack/react-query'
import { act, render } from '@testing-library/react'
import { invoke, type Channel } from '@tauri-apps/api/core'
import { vi } from 'vitest'
import { queryClient } from '../app/queryClient'
import { mirrorRust } from '../app/mirror'
import type { Patch, PillSettings, SettingsView } from '../shared/bindings'

/** Render against the app's own client, which event handlers also write to. */
export function renderWithQuery(ui: ReactElement) {
  queryClient.clear()
  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>)
}

const DEFAULT_SETTINGS: SettingsView = {
  activeModel: 'whisper-tiny',
  hotkeys: { pushToTalk: null, dictation: null, dictationCommit: null },
  language: { supported: false, options: [] },
  modelUnload: 'never',
  inputDevice: null,
  keepMicOpen: false,
  savedAudio: { enabled: true, keep: 10 },
  pasteMode: 'auto',
  formatter: { enabled: false, provider: 'ollama', profiles: {} },
  pill: { theme: 'steel', waveformStyle: 'bars', liveTranscript: false, startSound: false },
}

/** The settings slice at `rev`, changed from the defaults by `change`. */
export const settingsAt = (rev: number, change: Partial<SettingsView> = {}): Patch =>
  ({ slice: 'settings', rev, value: { ...DEFAULT_SETTINGS, ...change } })

/** The settings slice at `rev`, with `pill` changed from the defaults. */
export const pillAt = (rev: number, pill: Partial<PillSettings>): Patch =>
  settingsAt(rev, { pill: { ...DEFAULT_SETTINGS.pill, ...pill } })

/** Every slice as a fresh install starts. */
const FRESH: Patch[] = [
  settingsAt(0),
  { slice: 'hotkeys', rev: 0, value: DEFAULT_SETTINGS.hotkeys },
  { slice: 'models', rev: 0, value: [] },
  { slice: 'downloads', rev: 0, value: [] },
  { slice: 'cuda', rev: 0, value: null },
  { slice: 'recording', rev: 0, value: { status: 'idle', deferred: [] } },
  { slice: 'jobs', rev: 0, value: [] },
  { slice: 'dictionary', rev: 0, value: [] },
  { slice: 'savedAudio', rev: 0, value: 0 },
]

/** Mirror Rust's state, as each window does before its first render: `given`,
 *  and a fresh install's value of every other slice. Returns how Rust sends
 *  this window a later patch. */
export async function mirror(given: Patch[] = []) {
  const snapshot = [...given, ...FRESH.filter(patch => !given.some(g => g.slice === patch.slice))]
  let channel: Channel<Patch> | undefined
  vi.mocked(invoke).mockImplementationOnce((_cmd, args) => {
    channel = (args as { patches: Channel<Patch> }).patches
    return Promise.resolve(snapshot)
  })
  await mirrorRust()
  return async (patch: Patch) => { await act(async () => { channel!.onmessage(patch) }) }
}
