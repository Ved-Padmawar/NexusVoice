import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { KeepMicOpenToggle } from '../../pages/settings/KeepMicOpenToggle'
import { invoke } from '@tauri-apps/api/core'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

beforeEach(() => {
  mockInvoke.mockReset()
})

describe('KeepMicOpenToggle', () => {
  it('shows the saved setting', async () => {
    mockInvoke.mockResolvedValue(true)
    render(<KeepMicOpenToggle />)
    await waitFor(() => expect(screen.getByRole('switch')).toBeChecked())
  })

  it('persists a change via set_keep_mic_open', async () => {
    mockInvoke.mockResolvedValue(false)
    render(<KeepMicOpenToggle />)
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('get_keep_mic_open'))

    fireEvent.click(screen.getByRole('switch'))

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('set_keep_mic_open', { keep: true }))
    expect(screen.getByRole('switch')).toBeChecked()
  })
})
