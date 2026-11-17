import { describe, it, expect, vi, beforeEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { startDownload } from '../../../features/models/queries'

const mockInvoke = vi.mocked(invoke)

beforeEach(() => {
  mockInvoke.mockReset()
})

describe('model downloads', () => {
  it('reports a refused download as not started', async () => {
    mockInvoke.mockRejectedValue({ code: 'unknown_model', message: 'That model isn’t available.' })
    expect(await startDownload('nope')).toBe(false)
  })
})
