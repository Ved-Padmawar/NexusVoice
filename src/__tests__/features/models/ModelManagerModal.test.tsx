import { describe, it, expect, vi } from 'vitest'
import { fireEvent, screen, waitFor } from '@testing-library/react'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { ModelManagerModal } from '../../../features/models/ModelManagerModal'
import { renderWithQuery } from '../../utils'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

describe('ModelManagerModal', () => {
  it('shows why Rust refused a delete', async () => {
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === 'get_downloaded_models') return Promise.resolve([{ variant: 'whisper-tiny', displayName: 'Whisper Tiny', sizeBytes: 44_000_000, isActive: false }])
      if (cmd === 'get_model_catalog') return Promise.resolve([])
      if (cmd === 'delete_model') return Promise.reject({ code: 'io_error', message: 'The file is in use by another process' })
      return Promise.resolve(null)
    })
    renderWithQuery(<ModelManagerModal open onClose={() => {}} />)
    fireEvent.click(await screen.findByRole('button', { name: 'Delete Whisper Tiny' }))
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('The file is in use by another process'))
  })
})
