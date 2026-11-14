import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen, fireEvent, waitFor } from '@testing-library/react'
import { KeepMicOpenToggle } from '../../../features/settings/KeepMicOpenToggle'
import { invoke } from '@tauri-apps/api/core'
import { renderWithQuery } from '../../utils'

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

/** Rust's saved value; a save that `fails` leaves it alone. */
function backend(saved: boolean, { fails = false } = {}) {
  mockInvoke.mockImplementation((cmd, args) => {
    if (cmd !== 'set_keep_mic_open') return Promise.resolve(saved)
    if (fails) return Promise.reject({ code: 'io', message: 'disk full' })
    saved = (args as { keep: boolean }).keep
    return Promise.resolve(null)
  })
}

beforeEach(() => {
  mockInvoke.mockReset()
})

describe('KeepMicOpenToggle', () => {
  it('shows the saved setting', async () => {
    backend(true)
    renderWithQuery(<KeepMicOpenToggle />)
    await waitFor(() => expect(screen.getByRole('switch')).toBeChecked())
  })

  it('persists a change via set_keep_mic_open', async () => {
    backend(false)
    renderWithQuery(<KeepMicOpenToggle />)
    await waitFor(() => expect(screen.getByRole('switch')).toBeEnabled())

    fireEvent.click(screen.getByRole('switch'))

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('set_keep_mic_open', { keep: true }))
    await waitFor(() => expect(screen.getByRole('switch')).toBeChecked())
  })

  it('shows the saved setting again when a save fails', async () => {
    backend(false, { fails: true })
    renderWithQuery(<KeepMicOpenToggle />)
    await waitFor(() => expect(screen.getByRole('switch')).toBeEnabled())

    fireEvent.click(screen.getByRole('switch'))

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('set_keep_mic_open', { keep: true }))
    await waitFor(() => expect(screen.getByRole('switch')).not.toBeChecked())
  })
})
