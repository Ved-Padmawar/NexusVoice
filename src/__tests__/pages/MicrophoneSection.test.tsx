import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { MicrophoneSection } from '../../pages/settings/MicrophoneSection'
import { invoke } from '@tauri-apps/api/core'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

const DEVICES = [
  { name: 'Realtek Microphone', isDefault: true, isSelected: false },
  { name: 'Blue Yeti', isDefault: false, isSelected: false },
]

beforeEach(() => {
  mockInvoke.mockReset()
})

describe('MicrophoneSection', () => {
  it('lists named devices alongside the default option', async () => {
    mockInvoke.mockResolvedValue(DEVICES)
    render(<MicrophoneSection />)

    // The trigger shows the default device once loaded.
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Microphone' })).toHaveTextContent(/Default — Realtek Microphone/)
    )

    // Opening the menu reveals the named alternative.
    fireEvent.click(screen.getByRole('button', { name: 'Microphone' }))
    expect(await screen.findByRole('listbox')).toHaveTextContent('Blue Yeti')
  })

  it('preselects a saved non-default device', async () => {
    mockInvoke.mockResolvedValue([
      { name: 'Realtek Microphone', isDefault: true, isSelected: false },
      { name: 'Blue Yeti', isDefault: false, isSelected: true },
    ])
    render(<MicrophoneSection />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Microphone' })).toHaveTextContent('Blue Yeti')
    )
  })

  it('persists the chosen device via set_input_device', async () => {
    mockInvoke.mockResolvedValue(DEVICES)
    render(<MicrophoneSection />)

    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Microphone' })).toHaveTextContent(/Default — Realtek Microphone/)
    )
    fireEvent.click(screen.getByRole('button', { name: 'Microphone' }))
    fireEvent.click(await screen.findByText('Blue Yeti'))

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('set_input_device', { name: 'Blue Yeti' })
    )
  })
})
