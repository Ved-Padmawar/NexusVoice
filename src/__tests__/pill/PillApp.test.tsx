import { Profiler, StrictMode } from 'react'
import { act, cleanup, render, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { listen, type EventCallback } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { PillApp } from '../../pill/PillApp'
import type { PillSettings } from '../../types'

vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ setAlwaysOnTop: vi.fn().mockResolvedValue(undefined), startDragging: vi.fn().mockResolvedValue(undefined) }) }))
vi.mock('../../components/WaveformCanvas', () => ({ WaveformCanvas: () => <canvas /> }))

const listeners = new Map<string, Set<EventCallback<unknown>>>()
const emit = async (event: string, payload: unknown = null) => {
  await act(async () => { for (const handler of listeners.get(event) ?? []) await handler({ event, id: 1, payload }) })
}

const DEFAULT_PILL: PillSettings = { theme: 'steel', waveformStyle: 'bars', liveTranscript: false }
let revision = 0
let recordingRevision = 0
const record = (status: string) => emit('recording-state-changed', { revision: ++recordingRevision, status })
/** A saved change to the pill's preferences, as the backend announces it. */
const setPill = (change: Partial<PillSettings>) =>
  emit('pill-settings-changed', { revision: ++revision, settings: { ...DEFAULT_PILL, ...change } })

const DELIVERED = { pasteError: null, saved: true }

const RESPONSES: Record<string, unknown> = {
  get_recording_state: { revision: 0, status: 'idle' },
  get_model_info: { downloaded: true, downloading: false },
  get_pill_settings: { revision: 0, settings: DEFAULT_PILL },
}
beforeEach(() => {
  listeners.clear()
  localStorage.clear()
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
  revision = 0
  recordingRevision = 0
  vi.mocked(invoke).mockReset().mockImplementation(cmd => Promise.resolve(RESPONSES[cmd]))
  vi.mocked(listen).mockReset().mockImplementation(async (event, handler) => {
    const callbacks = listeners.get(event) ?? new Set()
    callbacks.add(handler as EventCallback<unknown>)
    listeners.set(event, callbacks)
    return () => { callbacks.delete(handler as EventCallback<unknown>) }
  })
})
afterEach(() => { cleanup(); vi.unstubAllGlobals(); vi.restoreAllMocks() })

it.each([44, 200])('keeps a transcript of height %i in its card layout until collapse finishes', async innerHeight => {
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(innerHeight)
  const view = render(<PillApp />)
  await waitFor(() => expect(listeners.get('recording-state-changed')?.size).toBe(1))
  await setPill({ liveTranscript: true })
  await record('recording')
  await emit('transcription-partial', { committed: 'Live transcript', tentative: '' })
  const pill = view.getByRole('status')
  await waitFor(() => expect(parseFloat(pill.style.height)).toBeGreaterThan(50))
  vi.mocked(invoke).mockClear()
  await record('processing')
  expect(pill).toHaveAttribute('aria-label', 'NexusVoice: processing')
  expect(pill).toHaveClass('pill--expanded')
  expect(pill.querySelector('.pill__strip')).toHaveAttribute('aria-hidden', 'true')
  expect(invoke).not.toHaveBeenCalledWith('resize_pill', { expanded: false })
  await waitFor(() => expect(pill).not.toHaveClass('pill--expanded'), { timeout: 2000 })
  expect(invoke).toHaveBeenCalledWith('resize_pill', { expanded: false })
  expect(pill.querySelector('.pill__spinner')).toBeInTheDocument()
})

it('keeps outgoing words available during the transcript exit fade', async () => {
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(44)
  const view = render(<PillApp />)
  await waitFor(() => expect(listeners.get('recording-state-changed')?.size).toBe(1))
  await setPill({ liveTranscript: true })
  await record('recording')
  await emit('transcription-partial', { committed: 'Keep these words', tentative: 'visible' })
  await waitFor(() => expect(view.getByText('Keep these words')).toBeInTheDocument())
  await waitFor(() => expect(view.container.querySelector('.pill__transcript')).toHaveStyle({ opacity: '1' }))
  await record('processing')
  expect(view.getByText('Keep these words')).toBeInTheDocument()
  expect(view.getByText('visible')).toBeInTheDocument()
})

it('shows a microphone warning without ending the recording', async () => {
  const view = render(<PillApp />)
  await waitFor(() => expect(listeners.get('transcription-warning')?.size).toBe(1))
  await record('recording')
  vi.mocked(invoke).mockClear()
  await emit('transcription-warning', 'Your microphone stopped.')
  // The capsule window has no room above the pill, so it grows first.
  expect(await view.findByText('Your microphone stopped.')).toBeInTheDocument()
  expect(invoke).toHaveBeenCalledWith('resize_pill', { expanded: true })
  expect(view.getByRole('status')).toHaveAttribute('aria-label', 'NexusVoice: recording')
  await waitFor(() => expect(view.queryByText('Your microphone stopped.')).toBeNull(), { timeout: 4000 })
  await waitFor(() => expect(invoke).toHaveBeenLastCalledWith('resize_pill', { expanded: false }))
})

it('leaves pasting to the backend and returns to idle when it is done', async () => {
  const view = render(<PillApp />)
  await waitFor(() => expect(listeners.get('transcription-complete')?.size).toBe(1))
  await record('recording')
  await record('processing')
  vi.mocked(invoke).mockClear()
  await emit('transcription-complete', DELIVERED)
  await record('idle')
  expect(view.getByRole('status')).toHaveAttribute('aria-label', 'NexusVoice: idle')
  expect(invoke).not.toHaveBeenCalledWith('type_text', expect.anything())
})

it.each(['push-to-talk', 'dictation'])('receives %s completion during renders even when listener registration IPC is slow', async mode => {
  const register = vi.mocked(listen).getMockImplementation()!
  const pending: (() => void)[] = []
  let completions = 0
  vi.mocked(listen).mockImplementation((event, handler) => {
    if (event === 'transcription-complete' && ++completions > 1) {
      return new Promise(resolve => {
        pending.push(() => { void register(event, handler).then(resolve) })
      })
    }
    return register(event, handler)
  })
  const view = render(<PillApp />)
  try {
    await waitFor(() => expect(listeners.get('transcription-complete')?.size).toBe(1))
    await record(mode === 'dictation' ? 'dictation' : 'recording')
    await record('processing')
    expect(view.getByRole('status')).toHaveAttribute('aria-label', 'NexusVoice: processing')
    // Rust has delivered the text; its one-shot event cannot wait for listen IPC.
    await emit('transcription-complete', DELIVERED)
  await record('idle')
    expect(view.getByRole('status')).toHaveAttribute('aria-label', 'NexusVoice: idle')
    expect(completions).toBe(1)
  } finally {
    await act(async () => { pending.forEach(resume => resume()) })
  }
})

it.each([
  ['transcription-complete', DELIVERED, 'idle'],
  ['transcription-error', 'Decode failed', 'error'],
])('keeps %s subscribed across repeated recordings and live preference renders', async (event, payload, expected) => {
  const view = render(<PillApp />)
  await waitFor(() => expect(listeners.get('recording-state-changed')?.size).toBe(1))
  for (let i = 0; i < 20; i++) {
    await record('recording')
    await setPill({ theme: i % 2 ? 'steel' : 'dawn' })
    await record('processing')
    await emit(event as string, payload)
    if (event === 'transcription-complete') await record('idle')
    expect(view.getByRole('status')).toHaveAttribute('aria-label', `NexusVoice: ${expected}`)
  }
  expect(vi.mocked(listen).mock.calls.filter(([name]) => name === event)).toHaveLength(1)
})

it('ignores a delayed snapshot after newer backend events and survives a renderer reload', async () => {
  let resolveSnapshot!: (value: unknown) => void
  vi.mocked(invoke).mockImplementation(cmd => cmd === 'get_recording_state'
    ? new Promise(resolve => { resolveSnapshot = resolve }) : Promise.resolve(RESPONSES[cmd]))
  const view = render(<PillApp />)
  await waitFor(() => expect(resolveSnapshot).toBeTypeOf('function'))
  await record('dictation-paused')
  await act(async () => resolveSnapshot({ revision: 0, status: 'idle' }))
  expect(view.getByRole('status')).toHaveAttribute('aria-label', 'NexusVoice: dictation-paused')
  view.unmount()
  vi.mocked(invoke).mockImplementation(cmd => Promise.resolve(cmd === 'get_recording_state'
    ? { revision: recordingRevision, status: 'dictation-paused' } : RESPONSES[cmd]))
  const reloaded = render(<PillApp />)
  await waitFor(() => expect(reloaded.getByRole('status')).toHaveAttribute('aria-label', 'NexusVoice: dictation-paused'))
  await record('processing')
  await record('idle')
  expect(reloaded.getByRole('status')).toHaveAttribute('aria-label', 'NexusVoice: idle')
})

it.each([
  [true, "Couldn't paste — it's saved in your history"],
  [false, "Couldn't paste or save this dictation"],
])('points to history after a failed paste only when saved is %s', async (saved, message) => {
  const view = render(<PillApp />)
  await waitFor(() => expect(listeners.get('transcription-complete')?.size).toBe(1))
  await record('recording')
  await record('processing')
  await emit('transcription-complete', { pasteError: 'clipboard locked', saved })
  expect(await view.findByText(message)).toBeInTheDocument()
})

it('ignores a preferences update older than the one it shows', async () => {
  const view = render(<PillApp />)
  await waitFor(() => expect(listeners.get('pill-settings-changed')?.size).toBe(1))
  await emit('pill-settings-changed', { revision: 5, settings: { ...DEFAULT_PILL, theme: 'aurora' } })
  await emit('pill-settings-changed', { revision: 4, settings: { ...DEFAULT_PILL, theme: 'dawn' } })
  expect(view.getByRole('status')).toHaveAttribute('data-pill-theme', 'aurora')
})

it('shrinks the window even when recording ends before expansion IPC resolves', async () => {
  let finishExpansion: (() => void) | undefined
  vi.mocked(invoke).mockImplementation((cmd, args) => {
    if (cmd === 'get_model_info') return Promise.resolve({ downloaded: true, downloading: false })
    if (cmd === 'resize_pill' && (args as { expanded: boolean }).expanded) {
      return new Promise<void>(resolve => { finishExpansion = resolve })
    }
    return Promise.resolve(RESPONSES[cmd])
  })
  const view = render(<PillApp />)
  await waitFor(() => expect(listeners.get('recording-state-changed')?.size).toBe(1))
  await setPill({ liveTranscript: true })
  await record('recording')
  await emit('transcription-partial', { committed: 'Short recording', tentative: '' })
  await waitFor(() => expect(finishExpansion).toBeTypeOf('function'))
  vi.mocked(invoke).mockClear()
  await record('processing')
  await emit('transcription-complete', DELIVERED)
  await record('idle')
  await act(async () => finishExpansion!())
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('resize_pill', { expanded: false }))
  expect(view.getByRole('status')).not.toHaveClass('pill--expanded')
})

it('does not let a cancelled close shrink a reopened card', async () => {
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(100)
  const view = render(<PillApp />)
  await waitFor(() => expect(listeners.get('recording-state-changed')?.size).toBe(1))
  await setPill({ liveTranscript: true })
  await record('recording')
  await emit('transcription-partial', { committed: 'First recording', tentative: '' })
  await waitFor(() => expect(parseFloat(view.getByRole('status').style.height)).toBeGreaterThan(60))
  vi.mocked(invoke).mockClear()
  await record('processing')
  await emit('transcription-complete', DELIVERED)
  await record('idle')
  await record('recording')
  await emit('transcription-partial', { committed: 'Second recording', tentative: '' })
  await waitFor(() => expect(parseFloat(view.getByRole('status').style.width)).toBeCloseTo(332, 0))
  expect(invoke).not.toHaveBeenCalledWith('resize_pill', { expanded: false })
  await record('processing')
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('resize_pill', { expanded: false }))
})

it('restores capsule bounds on mount after a renderer reload', async () => {
  render(<PillApp />)
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('resize_pill', { expanded: false }))
})

it('stages idle content when a card returns directly to idle without processing', async () => {
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(100)
  const view = render(<PillApp />)
  await waitFor(() => expect(listeners.get('transcription-complete')?.size).toBe(1))
  await setPill({ liveTranscript: true })
  await record('recording')
  await emit('transcription-partial', { committed: 'Direct result', tentative: '' })
  const pill = view.getByRole('status')
  await waitFor(() => expect(parseFloat(pill.style.height)).toBeGreaterThan(100))
  await emit('transcription-complete', DELIVERED)
  await record('idle')
  expect(pill).toHaveAttribute('aria-label', 'NexusVoice: idle')
  expect(pill.querySelector('.pill__spinner')).not.toBeInTheDocument()
  expect(pill.querySelector('.pill__strip')).toHaveClass('pill__strip--returning')
  await waitFor(() => expect(pill).not.toHaveClass('pill--expanded'))
  expect(pill.querySelector('.pill__strip')).not.toHaveClass('pill__strip--returning')
  expect(view.getByText('NexusVoice')).toBeInTheDocument()
})

it('has one subscription per event after StrictMode remounts and cleans them all up', async () => {
  const view = render(<StrictMode><PillApp /></StrictMode>)
  await waitFor(() => expect(listeners.get('recording-state-changed')?.size).toBe(1))
  for (const callbacks of listeners.values()) expect(callbacks.size).toBe(1)
  view.unmount()
  // Each subscription resolves to its unlisten; let those run.
  await act(async () => {})
  for (const callbacks of listeners.values()) expect(callbacks.size).toBe(0)
})

it('unsubscribes registrations that resolve after unmount', async () => {
  const pending: (() => void)[] = []
  const unlisten = vi.fn()
  vi.mocked(listen).mockImplementation(() => new Promise(resolve => { pending.push(() => resolve(unlisten)) }))
  const view = render(<PillApp />)
  view.unmount()
  await act(async () => { pending.forEach(resolve => resolve()) })
  expect(pending.length).toBeGreaterThan(10)
  expect(unlisten).toHaveBeenCalledTimes(pending.length)
})

it.each(['bars', 'memo'] as const)('updates %s audio levels without rendering the pill or invoking commands', async style => {
  const onRender = vi.fn()
  const view = render(<Profiler id="pill" onRender={onRender}><PillApp /></Profiler>)
  await waitFor(() => expect(listeners.get('recording-state-changed')?.size).toBe(1))
  await setPill({ waveformStyle: style })
  await record('recording')
  onRender.mockClear()
  vi.mocked(invoke).mockClear()
  await emit('pill-waveform', Array(8).fill(1))
  await emit('pill-waveform', Array(8).fill(1))
  expect(onRender).not.toHaveBeenCalled()
  expect(invoke).not.toHaveBeenCalled()
  if (style === 'bars') for (const bar of view.container.querySelectorAll('.pill__bar')) expect(bar).toHaveStyle({ height: '16px' })
})

it('keeps a newer recording visible when late delivery or model events arrive', async () => {
  const view = render(<PillApp />)
  await waitFor(() => expect(listeners.get('recording-state-changed')?.size).toBe(1))
  await record('recording')
  await record('processing')
  await record('idle')
  await record('recording')
  await emit('transcription-complete', DELIVERED)
  await emit('model-download-complete')
  expect(view.getByRole('status')).toHaveAttribute('aria-label', 'NexusVoice: recording')
})

it('clears a capture error after the backend returns to idle', async () => {
  const view = render(<PillApp />)
  await waitFor(() => expect(listeners.get('recording-state-changed')?.size).toBe(1))
  await record('recording')
  await emit('transcription-error', 'Mic access denied')
  await record('idle')
  expect(view.getByRole('status')).toHaveAttribute('aria-label', 'NexusVoice: error')
  await waitFor(() => expect(view.getByRole('status')).toHaveAttribute('aria-label', 'NexusVoice: idle'), { timeout: 4000 })
})
