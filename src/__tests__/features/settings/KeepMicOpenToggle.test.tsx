import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { KeepMicOpenToggle } from '../../../features/settings/KeepMicOpenToggle'
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
    await waitFor(() => expect(screen.getByRole('switch')).toBeEnabled())

    fireEvent.click(screen.getByRole('switch'))

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('set_keep_mic_open', { keep: true }))
    await waitFor(() => expect(screen.getByRole('switch')).toBeChecked())
  })

  it('keeps showing the confirmed setting when a save fails', async () => {
    mockInvoke.mockImplementation((cmd) =>
      cmd === 'set_keep_mic_open' ? Promise.reject({ code: 'io', message: 'disk full' }) : Promise.resolve(false))
    render(<KeepMicOpenToggle />)
    await waitFor(() => expect(screen.getByRole('switch')).toBeEnabled())

    fireEvent.click(screen.getByRole('switch'))

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('set_keep_mic_open', { keep: true }))
    await waitFor(() => expect(screen.getByRole('switch')).toBeEnabled())
    expect(screen.getByRole('switch')).not.toBeChecked()
  })
})
