/**
 * Auto-update. Nothing here surfaces an error on its own — a broken progress
 * calculation just shows a stuck or NaN bar, and a missing re-entrancy guard
 * starts two installs.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { useAppStore } from '../../store/useAppStore'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }))
vi.mock('@tauri-apps/plugin-updater', () => ({ check: vi.fn() }))
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }))

const mockCheck = vi.mocked(check)
const mockRelaunch = vi.mocked(relaunch)

type Progress =
  | { event: 'Started'; data: { contentLength?: number } }
  | { event: 'Progress'; data: { chunkLength: number } }
  | { event: 'Finished'; data: Record<string, never> }

/** A fake updater handle that replays `events` through the progress callback. */
const handle = (version: string, events: Progress[] = []) => ({
  available: true,
  version,
  downloadAndInstall: vi.fn(async (onProgress: (p: Progress) => void) => {
    for (const e of events) onProgress(e)
  }),
})

const state = () => useAppStore.getState()

beforeEach(() => {
  vi.clearAllMocks()
  useAppStore.setState({
    updateStatus: 'idle',
    updateVersion: null,
    updateProgress: 0,
    updateError: null,
    updateDismissed: false,
  })
})

describe('checkForUpdate', () => {
  it('reports an available update and its version', async () => {
    mockCheck.mockResolvedValue(handle('1.17.0') as never)
    await state().checkForUpdate()
    expect(state().updateStatus).toBe('available')
    expect(state().updateVersion).toBe('1.17.0')
  })

  it('reports up-to-date when the updater returns nothing', async () => {
    mockCheck.mockResolvedValue(null as never)
    await state().checkForUpdate()
    expect(state().updateStatus).toBe('up-to-date')
    expect(state().updateVersion).toBeNull()
  })

  it('treats an unavailable update as up-to-date, not as an offer', async () => {
    mockCheck.mockResolvedValue({ available: false, version: '1.17.0' } as never)
    await state().checkForUpdate()
    expect(state().updateStatus).toBe('up-to-date')
    expect(state().updateVersion).toBeNull()
  })

  it('records a check failure without throwing', async () => {
    // No network on a plane must not crash the About tab.
    mockCheck.mockRejectedValue({ message: 'network unreachable' })
    await expect(state().checkForUpdate()).resolves.toBeUndefined()
    expect(state().updateStatus).toBe('error')
    expect(state().updateError).toBe('network unreachable')
  })

  it('clears a previous error when a new check starts', async () => {
    mockCheck.mockRejectedValueOnce({ message: 'first failure' })
    await state().checkForUpdate()
    expect(state().updateError).toBe('first failure')

    mockCheck.mockResolvedValue(handle('1.17.0') as never)
    await state().checkForUpdate()
    expect(state().updateError).toBeNull()
    expect(state().updateStatus).toBe('available')
  })
})

describe('installUpdate', () => {
  it('does nothing when no update has been found', async () => {
    // `installUpdate` with no handle must be inert, not throw.
    await expect(state().installUpdate()).resolves.toBeUndefined()
  })

  it('reports download progress as a percentage of the total', async () => {
    mockCheck.mockResolvedValue(
      handle('1.17.0', [
        { event: 'Started', data: { contentLength: 1000 } },
        { event: 'Progress', data: { chunkLength: 250 } },
        { event: 'Progress', data: { chunkLength: 250 } },
      ]) as never,
    )
    await state().checkForUpdate()
    await state().installUpdate()
    // Chunks accumulate: 250 + 250 of 1000.
    expect(state().updateProgress).toBe(50)
  })

  it('marks the update ready and full when the download finishes', async () => {
    mockCheck.mockResolvedValue(
      handle('1.17.0', [
        { event: 'Started', data: { contentLength: 400 } },
        { event: 'Progress', data: { chunkLength: 400 } },
        { event: 'Finished', data: {} },
      ]) as never,
    )
    await state().checkForUpdate()
    await state().installUpdate()
    expect(state().updateStatus).toBe('ready')
    expect(state().updateProgress).toBe(100)
  })

  it('leaves progress at zero when the server sends no content length', async () => {
    // Dividing by a zero total would set the bar to NaN, which renders as a
    // blank or full bar depending on the browser.
    mockCheck.mockResolvedValue(
      handle('1.17.0', [
        { event: 'Started', data: {} },
        { event: 'Progress', data: { chunkLength: 500 } },
      ]) as never,
    )
    await state().checkForUpdate()
    await state().installUpdate()
    expect(state().updateProgress).toBe(0)
    expect(Number.isNaN(state().updateProgress)).toBe(false)
  })

  it('ignores a second install while one is downloading', async () => {
    // Two concurrent downloadAndInstall calls race over the same file.
    let release!: () => void
    const blocked = {
      available: true,
      version: '1.17.0',
      downloadAndInstall: vi.fn(() => new Promise<void>(r => { release = r })),
    }
    mockCheck.mockResolvedValue(blocked as never)
    await state().checkForUpdate()

    const first = state().installUpdate()
    await vi.waitFor(() => expect(state().updateStatus).toBe('downloading'))
    await state().installUpdate()

    expect(blocked.downloadAndInstall).toHaveBeenCalledTimes(1)
    release()
    await first
  })

  it('records a download failure and keeps the app usable', async () => {
    const failing = {
      available: true,
      version: '1.17.0',
      downloadAndInstall: vi.fn(() => Promise.reject({ message: 'checksum mismatch' })),
    }
    mockCheck.mockResolvedValue(failing as never)
    await state().checkForUpdate()
    await expect(state().installUpdate()).resolves.toBeUndefined()
    expect(state().updateStatus).toBe('error')
    expect(state().updateError).toBe('checksum mismatch')
  })
})

describe('restartForUpdate', () => {
  it('relaunches the app', async () => {
    mockRelaunch.mockResolvedValue(undefined)
    await state().restartForUpdate()
    expect(mockRelaunch).toHaveBeenCalledOnce()
  })

  it('reports a failed relaunch instead of leaving the user waiting', async () => {
    mockRelaunch.mockRejectedValue({ message: 'permission denied' })
    await expect(state().restartForUpdate()).resolves.toBeUndefined()
    expect(state().updateStatus).toBe('error')
    expect(state().updateError).toBe('permission denied')
  })
})

describe('dismissUpdate', () => {
  it('hides the sidebar prompt without discarding the pending update', async () => {
    // About must still offer the install after the prompt is dismissed.
    mockCheck.mockResolvedValue(handle('1.17.0') as never)
    await state().checkForUpdate()
    state().dismissUpdate()
    expect(state().updateDismissed).toBe(true)
    expect(state().updateStatus).toBe('available')
    expect(state().updateVersion).toBe('1.17.0')
  })
})
