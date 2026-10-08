import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { ModelUnloadSection } from '../../pages/settings/ModelUnloadSection'
import { COMMANDS } from '../../lib/commands'
import { invoke } from '@tauri-apps/api/core'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

beforeEach(() => {
  mockInvoke.mockReset()
})

describe('ModelUnloadSection', () => {
  it('shows the saved setting as checked', async () => {
    mockInvoke.mockResolvedValue('after15Minutes')
    render(<ModelUnloadSection />)
    await waitFor(() =>
      expect(screen.getByRole('radio', { name: '15 min' })).toHaveAttribute('aria-checked', 'true')
    )
  })

  it('persists the chosen setting', async () => {
    mockInvoke.mockResolvedValue('never')
    render(<ModelUnloadSection />)
    // Disabled until the saved value has loaded.
    await waitFor(() => expect(screen.getByRole('radio', { name: '1 hour' })).toBeEnabled())
    fireEvent.click(screen.getByRole('radio', { name: '1 hour' }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(COMMANDS.SET_MODEL_UNLOAD, { policy: 'after1Hour' })
    )
  })
})
