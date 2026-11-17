import type { ReactElement } from 'react'
import { QueryClientProvider } from '@tanstack/react-query'
import { act, render } from '@testing-library/react'
import { invoke, type Channel } from '@tauri-apps/api/core'
import { vi } from 'vitest'
import { queryClient } from '../app/queryClient'
import { ALL_SLICES, mirrorRust } from '../app/mirror'
import type { Change, Patch, PillSettings, SettingsView } from '../shared/bindings'

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

/** A slice's value as a commit at `rev` sent it. */
export type PatchAt = Patch & { rev: number }

/** The settings slice at `rev`, changed from the defaults by `change`. */
export const settingsAt = (rev: number, change: Partial<SettingsView> = {}): PatchAt =>
  ({ slice: 'settings', rev, value: { ...DEFAULT_SETTINGS, ...change } })

/** The settings slice at `rev`, with `pill` changed from the defaults. */
export const pillAt = (rev: number, pill: Partial<PillSettings>): PatchAt =>
  settingsAt(rev, { pill: { ...DEFAULT_SETTINGS.pill, ...pill } })

/** Every slice as a fresh install starts. */
const FRESH: PatchAt[] = [
  settingsAt(0),
  { slice: 'hotkeys', rev: 0, value: DEFAULT_SETTINGS.hotkeys },
  { slice: 'models', rev: 0, value: [] },
  { slice: 'downloads', rev: 0, value: [] },
  { slice: 'cuda', rev: 0, value: null },
  { slice: 'recording', rev: 0, value: { status: 'idle', deferred: [] } },
  { slice: 'jobs', rev: 0, value: [] },
  { slice: 'dictionary', rev: 0, value: [] },
  { slice: 'savedAudio', rev: 0, value: { bytes: 0, cleanupFailed: false } },
  { slice: 'history', rev: 0, value: 0 },
  { slice: 'historyTotals', rev: 0, value: 0 },
]

/** How a test sends a window one slice's commit. */
export type Send = (patch: PatchAt) => Promise<void>

const changeOf = ({ rev, ...patch }: PatchAt): Change => ({ rev, patches: [patch as Patch] })

/** Mirror Rust's state, as each window does before its first render: `given`,
 *  and a fresh install's value of every other slice. Returns how Rust sends
 *  this window a later commit of one slice. */
export async function mirror(given: PatchAt[] = []): Promise<Send> {
  const all = [...given, ...FRESH.filter(patch => !given.some(g => g.slice === patch.slice))]
  const snapshot: Change = { rev: Math.max(...all.map(p => p.rev)), patches: all.map(p => changeOf(p).patches[0]) }
  let channel: Channel<Change> | undefined
  vi.mocked(invoke).mockImplementationOnce((_cmd, args) => {
    channel = (args as { changes: Channel<Change> }).changes
    return Promise.resolve(snapshot)
  })
  await mirrorRust(ALL_SLICES)
  return async (patch: PatchAt) => { await act(async () => { channel!.onmessage(changeOf(patch)) }) }
}
