import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { listen } from '@tauri-apps/api/event'
import { useDelayedFlag, useEventListener } from '../../shared/hooks'

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

beforeEach(() => { vi.useFakeTimers() })
afterEach(() => { vi.useRealTimers(); vi.clearAllMocks() })

describe('useDelayedFlag', () => {
  it('stays false until the delay has fully elapsed', () => {
    // The whole point: a sub-second wait must never flash a spinner.
    const { result } = renderHook(() => useDelayedFlag(true, 250))
    expect(result.current).toBe(false)
    act(() => { vi.advanceTimersByTime(249) })
    expect(result.current).toBe(false)
    act(() => { vi.advanceTimersByTime(1) })
    expect(result.current).toBe(true)
  })

  it('never fires when the wait finishes before the delay', () => {
    const { result, rerender } = renderHook(({ active }) => useDelayedFlag(active, 250), {
      initialProps: { active: true },
    })
    act(() => { vi.advanceTimersByTime(100) })
    rerender({ active: false })
    act(() => { vi.advanceTimersByTime(1000) })
    expect(result.current).toBe(false)
  })

  it('clears immediately when the wait ends, without a trailing delay', () => {
    // A spinner that lingers after the data arrives reads as a hang.
    const { result, rerender } = renderHook(({ active }) => useDelayedFlag(active, 250), {
      initialProps: { active: true },
    })
    act(() => { vi.advanceTimersByTime(300) })
    expect(result.current).toBe(true)
    rerender({ active: false })
    expect(result.current).toBe(false)
  })

  it('restarts the delay for a second wait', () => {
    const { result, rerender } = renderHook(({ active }) => useDelayedFlag(active, 250), {
      initialProps: { active: true },
    })
    act(() => { vi.advanceTimersByTime(300) })
    rerender({ active: false })
    rerender({ active: true })
    expect(result.current).toBe(false)
    act(() => { vi.advanceTimersByTime(250) })
    expect(result.current).toBe(true)
  })

  it('is false from the first render when nothing is pending', () => {
    const { result } = renderHook(() => useDelayedFlag(false))
    act(() => { vi.advanceTimersByTime(5000) })
    expect(result.current).toBe(false)
  })
})

describe('useEventListener', () => {
  it('subscribes once and unsubscribes on unmount', async () => {
    const unlisten = vi.fn()
    vi.mocked(listen).mockResolvedValue(unlisten)

    const { unmount } = renderHook(() => useEventListener('transcriptionComplete', () => {}))
    expect(listen).toHaveBeenCalledTimes(1)
    expect(vi.mocked(listen).mock.calls[0][0]).toBe('transcription-complete')

    unmount()
    await act(async () => { await vi.runAllTimersAsync() })
    expect(unlisten).toHaveBeenCalledTimes(1)
  })

  it('does not resubscribe when only the handler changes', async () => {
    // The handler is held in a ref precisely so an inline arrow — a new function
    // every render — does not tear down and rebuild the subscription.
    vi.mocked(listen).mockResolvedValue(vi.fn())
    const { rerender } = renderHook(({ n }) => useEventListener('cudaDownloadProgress', () => n), {
      initialProps: { n: 1 },
    })
    rerender({ n: 2 })
    rerender({ n: 3 })
    expect(listen).toHaveBeenCalledTimes(1)
  })

  it('calls the latest handler, not the one captured at subscribe time', async () => {
    // The flip side of the ref: a stale handler silently acts on old state.
    let received = 0
    vi.mocked(listen).mockResolvedValue(vi.fn())
    const { rerender } = renderHook(({ n }) => useEventListener('cudaDownloadProgress', () => { received = n }), {
      initialProps: { n: 1 },
    })
    rerender({ n: 99 })

    const tauriHandler = vi.mocked(listen).mock.calls[0][1] as (e: { payload: number }) => void
    act(() => tauriHandler({ payload: 0 }))
    expect(received).toBe(99)
  })

  it('passes the payload through, not the whole event envelope', async () => {
    const handler = vi.fn()
    vi.mocked(listen).mockResolvedValue(vi.fn())
    renderHook(() => useEventListener('modelDownloadProgress', handler))

    const tauriHandler = vi.mocked(listen).mock.calls[0][1] as (e: { payload: unknown }) => void
    act(() => tauriHandler({ payload: { id: 'model', pct: 42 } }))
    expect(handler).toHaveBeenCalledWith({ id: 'model', pct: 42 })
  })

  it('resubscribes when the event changes', async () => {
    vi.mocked(listen).mockResolvedValue(vi.fn())
    const { rerender } = renderHook(({ evt }: { evt: 'transcriptionComplete' | 'transcriptionError' }) => useEventListener(evt, () => {}), {
      initialProps: { evt: 'transcriptionComplete' },
    })
    rerender({ evt: 'transcriptionError' })
    expect(listen).toHaveBeenCalledTimes(2)
    expect(vi.mocked(listen).mock.calls[1][0]).toBe('transcription-error')
  })

  it('swallows a rejected subscription instead of an unhandled rejection', async () => {
    // `listen` can reject if the window is tearing down; that must not surface.
    vi.mocked(listen).mockRejectedValue(new Error('window gone'))
    const { unmount } = renderHook(() => useEventListener('transcriptionComplete', () => {}))
    unmount()
    await act(async () => { await vi.runAllTimersAsync() })
  })

  it('ignores callbacks after cleanup while registration is still pending', async () => {
    const handler = vi.fn()
    const unlisten = vi.fn()
    let registered!: (unlisten: () => void) => void
    vi.mocked(listen).mockImplementation(() => new Promise(resolve => { registered = resolve }))
    const { unmount } = renderHook(() => useEventListener('transcriptionComplete', handler))
    const callback = vi.mocked(listen).mock.calls[0][1]
    unmount()
    callback({ event: 'transcription-complete', id: 1, payload: { pasteError: null, saved: true } })
    await act(async () => { registered(unlisten) })
    expect(handler).not.toHaveBeenCalled()
    expect(unlisten).toHaveBeenCalledOnce()
  })

  it('handles registration failure immediately while still mounted', async () => {
    vi.mocked(listen).mockRejectedValue(new Error('window gone'))
    renderHook(() => useEventListener('transcriptionComplete', () => {}))
    await act(async () => { await vi.runAllTimersAsync() })
  })
})
