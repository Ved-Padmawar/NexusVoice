import { describe, it, expect, vi, beforeEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { relaunchApp } from '../../shared/relaunch'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { error: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

beforeEach(() => {
  vi.clearAllMocks()
})

describe('relaunchApp', () => {
  it('tells the user when Rust refuses because a dictation is running', async () => {
    mockInvoke.mockRejectedValue({ code: 'recording_in_progress', message: 'Finish the current dictation before restarting' })
    await relaunchApp()
    expect(toast.error).toHaveBeenCalledWith('Finish the current dictation before restarting')
  })
})
