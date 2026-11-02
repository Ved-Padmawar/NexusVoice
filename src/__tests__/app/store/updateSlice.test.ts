/**
 * Auto-update. Nothing here surfaces an error on its own — a broken progress
 * calculation just shows a stuck or NaN bar, and a missing re-entrancy guard
 * starts two installs.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { check } from '@tauri-apps/plugin-updater'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { useAppStore } from '../../../app/store/useAppStore'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }))
vi.mock('@tauri-apps/plugin-updater', () => ({ check: vi.fn() }))
vi.mock('sonner', () => ({ toast: { error: vi.fn() } }))

const mockCheck = vi.mocked(check)
const mockInvoke = vi.mocked(invoke)

const BUSY = { code: 'recording_in_progress', message: 'Finish the current dictation before restarting' }

type Progress =
  | { event: 'Started'; data: { contentLength?: number } }
  | { event: 'Progress'; data: { chunkLength: number } }
  | { event: 'Finished'; data: Record<string, never> }

/** A fake updater handle that replays `events` through the progress callback. */
const handle = (version: string, events: Progress[] = []) => ({
  available: true,
  version,
  download: vi.fn(async (onProgress: (p: Progress) => void) => {
    for (const e of events) onProgress(e)
  }),
  install: vi.fn(async () => {}),
})

const state = () => useAppStore.getState()

beforeEach(() => {
  vi.clearAllMocks()
  useAppStore.setState({
    updateStatus: 'idle',
    updateVersion: null,
    updateProgress: 0,
    updateError: null,
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

describe('downloadUpdate', () => {
  it('does nothing when no update has been found', async () => {
    // `downloadUpdate` with no handle must be inert, not throw.
    await expect(state().downloadUpdate()).resolves.toBeUndefined()
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
    await state().downloadUpdate()
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
    await state().downloadUpdate()
    expect(state().updateStatus).toBe('ready')
    expect(state().updateProgress).toBe(100)
  })

  it('is not ready until the download call itself resolves', async () => {
    // `Finished` is only an event; the download is complete when the call returns.
    let finishInstall!: () => void
    const pending = {
      available: true,
      version: '1.17.0',
      download: vi.fn(async (onProgress: (p: Progress) => void) => {
        onProgress({ event: 'Finished', data: {} })
        await new Promise<void>(r => { finishInstall = r })
      }),
    }
    mockCheck.mockResolvedValue(pending as never)
    await state().checkForUpdate()

    const install = state().downloadUpdate()
    await vi.waitFor(() => expect(state().updateProgress).toBe(100))
    expect(state().updateStatus).toBe('downloading')

    finishInstall()
    await install
    expect(state().updateStatus).toBe('ready')
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
    await state().downloadUpdate()
    expect(state().updateProgress).toBe(0)
    expect(Number.isNaN(state().updateProgress)).toBe(false)
  })

  it('ignores a second install while one is downloading', async () => {
    // Two concurrent download calls race over the same file.
    let release!: () => void
    const blocked = {
      available: true,
      version: '1.17.0',
      download: vi.fn(() => new Promise<void>(r => { release = r })),
    }
    mockCheck.mockResolvedValue(blocked as never)
    await state().checkForUpdate()

    const first = state().downloadUpdate()
    await vi.waitFor(() => expect(state().updateStatus).toBe('downloading'))
    await state().downloadUpdate()

    expect(blocked.download).toHaveBeenCalledTimes(1)
    release()
    await first
  })

  it('records a download failure and keeps the app usable', async () => {
    const failing = {
      available: true,
      version: '1.17.0',
      download: vi.fn(() => Promise.reject({ message: 'checksum mismatch' })),
    }
    mockCheck.mockResolvedValue(failing as never)
    await state().checkForUpdate()
    await expect(state().downloadUpdate()).resolves.toBeUndefined()
    expect(state().updateStatus).toBe('error')
    expect(state().updateError).toBe('checksum mismatch')
  })
})

describe('restartForUpdate', () => {
  it('holds new recordings in Rust, installs, then relaunches', async () => {
    const update = handle('1.17.0')
    mockCheck.mockResolvedValue(update as never)
    await state().checkForUpdate()
    mockInvoke.mockResolvedValue(undefined)

    await state().restartForUpdate()

    expect(mockInvoke.mock.calls.map(([cmd]) => cmd)).toEqual(['prepare_restart', 'relaunch_app'])
    expect(update.install).toHaveBeenCalledOnce()
    expect(mockInvoke.mock.invocationCallOrder[0]).toBeLessThan(update.install.mock.invocationCallOrder[0])
  })

  it('does not install while a dictation is running, and keeps the update ready', async () => {
    const update = handle('1.17.0')
    mockCheck.mockResolvedValue(update as never)
    await state().checkForUpdate()
    useAppStore.setState({ updateStatus: 'ready' })
    mockInvoke.mockRejectedValue(BUSY)

    await expect(state().restartForUpdate()).resolves.toBeUndefined()

    expect(update.install).not.toHaveBeenCalled()
    expect(state().updateStatus).toBe('ready')
    expect(toast.error).toHaveBeenCalledWith(BUSY.message)
  })

  it('reports a failed install and lets recordings start again', async () => {
    const update = { ...handle('1.17.0'), install: vi.fn(() => Promise.reject({ message: 'bad signature' })) }
    mockCheck.mockResolvedValue(update as never)
    await state().checkForUpdate()
    mockInvoke.mockResolvedValue(undefined)

    await state().restartForUpdate()

    expect(mockInvoke).toHaveBeenCalledWith('cancel_restart')
    expect(mockInvoke).not.toHaveBeenCalledWith('relaunch_app')
    expect(state().updateStatus).toBe('error')
    expect(state().updateError).toBe('bad signature')
  })

  it('installs once when Restart is clicked twice', async () => {
    const update = handle('1.17.0')
    mockCheck.mockResolvedValue(update as never)
    await state().checkForUpdate()
    mockInvoke.mockResolvedValue(undefined)

    await Promise.all([state().restartForUpdate(), state().restartForUpdate()])

    expect(update.install).toHaveBeenCalledOnce()
  })
})
