import type { ReactElement } from 'react'
import { QueryClientProvider } from '@tanstack/react-query'
import { act, render } from '@testing-library/react'
import { invoke, type Channel } from '@tauri-apps/api/core'
import { vi } from 'vitest'
import { queryClient } from '../app/queryClient'
import { mirrorRust } from '../app/mirror'
import type { Patch, PillSettings } from '../shared/bindings'

/** Render against the app's own client, which event handlers also write to. */
export function renderWithQuery(ui: ReactElement) {
  queryClient.clear()
  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>)
}

const DEFAULT_PILL: PillSettings = { theme: 'steel', waveformStyle: 'bars', liveTranscript: false, startSound: false }

/** The settings slice at `rev`, with `pill` changed from the defaults. */
export const settingsAt = (rev: number, pill: Partial<PillSettings> = {}): Patch =>
  ({ slice: 'settings', rev, value: { pill: { ...DEFAULT_PILL, ...pill } } })

/** Mirror `snapshot` as Rust's state, as each window does before its first
 *  render. Returns how Rust sends this window a later patch. */
export async function mirror(snapshot: Patch[] = [settingsAt(0)]) {
  let channel: Channel<Patch> | undefined
  vi.mocked(invoke).mockImplementationOnce((_cmd, args) => {
    channel = (args as { patches: Channel<Patch> }).patches
    return Promise.resolve(snapshot)
  })
  await mirrorRust()
  return async (patch: Patch) => { await act(async () => { channel!.onmessage(patch) }) }
}
