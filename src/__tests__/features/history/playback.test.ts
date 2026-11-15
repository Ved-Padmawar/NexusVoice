import { expect, it, vi } from 'vitest'
import { toast } from 'sonner'
import { stopIfPlaying, toggle, usePlayback } from '../../../features/history/playback'

vi.mock('sonner', () => ({ toast: { error: vi.fn() } }))
vi.mock('@tauri-apps/api/core', () => ({ convertFileSrc: (path: string) => path }))

it('ignores a superseded play rejection and reports a failure for the current file once', async () => {
  const players: FakeAudio[] = []
  class FakeAudio extends EventTarget {
    rejectPlay: (error: Error) => void = () => {}
    paused = true
    constructor() {
      super()
      players.push(this)
    }
    play() {
      this.paused = false
      return new Promise<void>((_, reject) => { this.rejectPlay = reject })
    }
    pause() { this.paused = true }
    removeAttribute() {}
  }
  vi.stubGlobal('Audio', FakeAudio)
  try {
    toggle(1, 'first.wav')
    toggle(2, 'second.wav')
    players[0].rejectPlay(new Error('superseded'))
    players[0].dispatchEvent(new Event('error'))
    await Promise.resolve()
    expect(usePlayback.getState().id).toBe(2)
    expect(toast.error).not.toHaveBeenCalled()

    players[1].rejectPlay(new Error('unreadable'))
    await Promise.resolve()
    players[1].dispatchEvent(new Event('error'))
    expect(usePlayback.getState().id).toBeNull()
    expect(toast.error).toHaveBeenCalledExactlyOnceWith('Could not play the saved audio.')
  } finally {
    stopIfPlaying(2)
    vi.unstubAllGlobals()
  }
})
