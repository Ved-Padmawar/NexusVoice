import { describe, it, expect, vi } from 'vitest'
import { fireEvent, screen, waitFor } from '@testing-library/react'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { ModelManagerModal } from '../../../features/models/ModelManagerModal'
import { mirror, renderWithQuery } from '../../utils'

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const mockBackend = async () => {
  vi.mocked(invoke).mockReset().mockImplementation((cmd) => {
    if (cmd === 'get_model_catalog') return Promise.resolve([])
    if (cmd === 'delete_model') return Promise.reject({ code: 'io_error', message: 'The file is in use by another process' })
    return Promise.resolve(null)
  })
  await mirror([{ slice: 'models', rev: 0, value: [{ variant: 'whisper-tiny', displayName: 'Whisper Tiny', sizeBytes: 44_000_000 }] }])
}

describe('ModelManagerModal', () => {
  it('asks for confirmation before deleting a model', async () => {
    await mockBackend()
    renderWithQuery(<ModelManagerModal open onClose={() => {}} />)
    fireEvent.click(await screen.findByRole('button', { name: 'Delete Whisper Tiny' }))
    expect(screen.getByRole('button', { name: 'Confirm deleting Whisper Tiny' })).toBeInTheDocument()
    expect(invoke).not.toHaveBeenCalledWith('delete_model', expect.anything())
  })

  it('shows why Rust refused a delete', async () => {
    await mockBackend()
    renderWithQuery(<ModelManagerModal open onClose={() => {}} />)
    fireEvent.click(await screen.findByRole('button', { name: 'Delete Whisper Tiny' }))
    fireEvent.click(screen.getByRole('button', { name: 'Confirm deleting Whisper Tiny' }))
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('The file is in use by another process'))
  })
})
