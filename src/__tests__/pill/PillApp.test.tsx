import { Profiler, StrictMode } from 'react'
import { act, cleanup, render, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { listen, type EventCallback } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { PillApp } from '../../pill/PillApp'
import { EVENTS } from '../../lib/events'

vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ setAlwaysOnTop: vi.fn().mockResolvedValue(undefined), startDragging: vi.fn().mockResolvedValue(undefined) }) }))
vi.mock('../../components/WaveformCanvas', () => ({ WaveformCanvas: () => <canvas /> }))

const listeners = new Map<string, Set<EventCallback<unknown>>>()
const emit = async (event: string, payload: unknown = null) => {
  await act(async () => { for (const handler of listeners.get(event) ?? []) await handler({ event, id: 1, payload }) })
}
beforeEach(() => {
  listeners.clear()
  localStorage.clear()
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
  vi.mocked(invoke).mockReset().mockImplementation(cmd => Promise.resolve(cmd === 'get_model_info' ? { downloaded: true, downloading: false } : undefined))
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
  await waitFor(() => expect(listeners.get(EVENTS.HOTKEY_RELEASED)?.size).toBe(1))
  await emit(EVENTS.PILL_LIVE_TRANSCRIPT_CHANGED, true)
  await emit(EVENTS.HOTKEY_PRESSED)
  await emit(EVENTS.TRANSCRIPTION_PARTIAL, { committed: 'Live transcript', tentative: '' })
  const pill = view.getByRole('status')
  await waitFor(() => expect(parseFloat(pill.style.height)).toBeGreaterThan(50))
  vi.mocked(invoke).mockClear()
  await emit(EVENTS.HOTKEY_RELEASED)
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
  await waitFor(() => expect(listeners.get(EVENTS.HOTKEY_RELEASED)?.size).toBe(1))
  await emit(EVENTS.PILL_LIVE_TRANSCRIPT_CHANGED, true)
  await emit(EVENTS.HOTKEY_PRESSED)
  await emit(EVENTS.TRANSCRIPTION_PARTIAL, { committed: 'Keep these words', tentative: 'visible' })
  await waitFor(() => expect(view.getByText('Keep these words')).toBeInTheDocument())
  await waitFor(() => expect(view.container.querySelector('.pill__transcript')).toHaveStyle({ opacity: '1' }))
  await emit(EVENTS.HOTKEY_RELEASED)
  expect(view.getByText('Keep these words')).toBeInTheDocument()
  expect(view.getByText('visible')).toBeInTheDocument()
})

it('shrinks the window even when recording ends before expansion IPC resolves', async () => {
  let finishExpansion: (() => void) | undefined
  vi.mocked(invoke).mockImplementation((cmd, args) => {
    if (cmd === 'get_model_info') return Promise.resolve({ downloaded: true, downloading: false })
    if (cmd === 'resize_pill' && (args as { expanded: boolean }).expanded) {
      return new Promise<void>(resolve => { finishExpansion = resolve })
    }
    return Promise.resolve()
  })
  const view = render(<PillApp />)
  await waitFor(() => expect(listeners.get(EVENTS.HOTKEY_RELEASED)?.size).toBe(1))
  await emit(EVENTS.PILL_LIVE_TRANSCRIPT_CHANGED, true)
  await emit(EVENTS.HOTKEY_PRESSED)
  await emit(EVENTS.TRANSCRIPTION_PARTIAL, { committed: 'Short recording', tentative: '' })
  await waitFor(() => expect(finishExpansion).toBeTypeOf('function'))
  vi.mocked(invoke).mockClear()
  await emit(EVENTS.HOTKEY_RELEASED)
  await emit(EVENTS.TRANSCRIPTION_COMPLETE, '')
  await act(async () => finishExpansion!())
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('resize_pill', { expanded: false }))
  expect(view.getByRole('status')).not.toHaveClass('pill--expanded')
})

it('does not let a cancelled close shrink a reopened card', async () => {
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(100)
  const view = render(<PillApp />)
  await waitFor(() => expect(listeners.get(EVENTS.HOTKEY_RELEASED)?.size).toBe(1))
  await emit(EVENTS.PILL_LIVE_TRANSCRIPT_CHANGED, true)
  await emit(EVENTS.HOTKEY_PRESSED)
  await emit(EVENTS.TRANSCRIPTION_PARTIAL, { committed: 'First recording', tentative: '' })
  await waitFor(() => expect(parseFloat(view.getByRole('status').style.height)).toBeGreaterThan(60))
  vi.mocked(invoke).mockClear()
  await emit(EVENTS.HOTKEY_RELEASED)
  await emit(EVENTS.TRANSCRIPTION_COMPLETE, '')
  await emit(EVENTS.HOTKEY_PRESSED)
  await emit(EVENTS.TRANSCRIPTION_PARTIAL, { committed: 'Second recording', tentative: '' })
  await waitFor(() => expect(parseFloat(view.getByRole('status').style.width)).toBeCloseTo(332, 0))
  expect(invoke).not.toHaveBeenCalledWith('resize_pill', { expanded: false })
  await emit(EVENTS.HOTKEY_RELEASED)
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('resize_pill', { expanded: false }))
})

it('restores capsule bounds on mount after a renderer reload', async () => {
  render(<PillApp />)
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('resize_pill', { expanded: false }))
})

it('stages idle content when a card returns directly to idle without processing', async () => {
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(100)
  const view = render(<PillApp />)
  await waitFor(() => expect(listeners.get(EVENTS.TRANSCRIPTION_COMPLETE)?.size).toBe(1))
  await emit(EVENTS.PILL_LIVE_TRANSCRIPT_CHANGED, true)
  await emit(EVENTS.HOTKEY_PRESSED)
  await emit(EVENTS.TRANSCRIPTION_PARTIAL, { committed: 'Direct result', tentative: '' })
  const pill = view.getByRole('status')
  await waitFor(() => expect(parseFloat(pill.style.height)).toBeGreaterThan(100))
  await emit(EVENTS.TRANSCRIPTION_COMPLETE, '')
  expect(pill).toHaveAttribute('aria-label', 'NexusVoice: idle')
  expect(pill.querySelector('.pill__spinner')).not.toBeInTheDocument()
  expect(pill.querySelector('.pill__strip')).toHaveClass('pill__strip--returning')
  await waitFor(() => expect(pill).not.toHaveClass('pill--expanded'))
  expect(pill.querySelector('.pill__strip')).not.toHaveClass('pill__strip--returning')
  expect(view.getByText('NexusVoice')).toBeInTheDocument()
})

it('has one subscription per event after StrictMode remounts and cleans them all up', async () => {
  const view = render(<StrictMode><PillApp /></StrictMode>)
  await waitFor(() => expect(listeners.get(EVENTS.DICTATION_COMMIT_HOTKEY_PRESSED)?.size).toBe(1))
  for (const callbacks of listeners.values()) expect(callbacks.size).toBe(1)
  view.unmount()
  for (const callbacks of listeners.values()) expect(callbacks.size).toBe(0)
})

it('unsubscribes registrations that resolve after unmount', async () => {
  const pending: (() => void)[] = []
  const unlisten = vi.fn()
  vi.mocked(listen).mockImplementation(() => new Promise(resolve => { pending.push(() => resolve(unlisten)) }))
  const view = render(<PillApp />)
  view.unmount()
  await act(async () => { pending.forEach(resolve => resolve()) })
  expect(unlisten).toHaveBeenCalledTimes(pending.length)
  expect(pending.length).toBe(5)
})

it.each(['bars', 'memo'])('updates %s audio levels without rendering the pill or invoking commands', async style => {
  const onRender = vi.fn()
  const view = render(<Profiler id="pill" onRender={onRender}><PillApp /></Profiler>)
  await waitFor(() => expect(listeners.get(EVENTS.HOTKEY_PRESSED)?.size).toBe(1))
  await emit(EVENTS.PILL_WAVEFORM_STYLE_CHANGED, style)
  await emit(EVENTS.HOTKEY_PRESSED)
  onRender.mockClear()
  vi.mocked(invoke).mockClear()
  await emit(EVENTS.PILL_WAVEFORM, Array(8).fill(1))
  await emit(EVENTS.PILL_WAVEFORM, Array(8).fill(1))
  expect(onRender).not.toHaveBeenCalled()
  expect(invoke).not.toHaveBeenCalled()
  if (style === 'bars') for (const bar of view.container.querySelectorAll('.pill__bar')) expect(bar).toHaveStyle({ height: '16px' })
})
