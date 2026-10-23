import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { setWindowVisible, subscribeWhileVisible } from '../../lib/frameClock'

describe('frameClock', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    setWindowVisible(true)
    vi.useRealTimers()
  })

  it('ticks at about 30 fps while subscribed, and not once cleaned up', () => {
    const onFrame = vi.fn()
    const stop = subscribeWhileVisible(document.body, onFrame)

    vi.advanceTimersByTime(1000)
    expect(onFrame.mock.calls.length).toBeGreaterThanOrEqual(29)
    expect(onFrame.mock.calls.length).toBeLessThanOrEqual(31)

    stop()
    onFrame.mockClear()
    vi.advanceTimersByTime(1000)
    expect(onFrame).not.toHaveBeenCalled()
  })

  it('stays silent while the window is hidden and resumes when shown', () => {
    const onFrame = vi.fn()
    const stop = subscribeWhileVisible(document.body, onFrame)

    setWindowVisible(false)
    vi.advanceTimersByTime(1000)
    expect(onFrame).not.toHaveBeenCalled()

    setWindowVisible(true)
    vi.advanceTimersByTime(1000)
    expect(onFrame).toHaveBeenCalled()
    stop()
  })
})
