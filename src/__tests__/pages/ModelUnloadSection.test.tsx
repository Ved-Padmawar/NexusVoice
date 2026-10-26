import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { ModelUnloadSection } from '../../pages/settings/ModelUnloadSection'
import { invoke } from '@tauri-apps/api/core'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

beforeEach(() => {
  mockInvoke.mockReset()
})

describe('ModelUnloadSection', () => {
  it('shows the saved setting selected', async () => {
    mockInvoke.mockResolvedValue('after15Minutes')
    render(<ModelUnloadSection />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Unload when idle' })).toHaveTextContent('15 min')
    )
  })

  it('persists the chosen setting', async () => {
    mockInvoke.mockResolvedValue('never')
    render(<ModelUnloadSection />)
    // Disabled until the saved value has loaded.
    await waitFor(() => expect(screen.getByRole('button', { name: 'Unload when idle' })).toBeEnabled())
    fireEvent.click(screen.getByRole('button', { name: 'Unload when idle' }))
    fireEvent.click(await screen.findByRole('option', { name: '1 hour' }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('set_model_unload', { policy: 'after1Hour' })
    )
  })
})
