/**
 * First-run onboarding stays open until the chosen model is on disk, even when
 * the download list has not been read yet.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { act, fireEvent, screen, waitFor } from '@testing-library/react'
import { invoke } from '@tauri-apps/api/core'
import { ModelPickerModal } from '../../../features/models/ModelPickerModal'
import { refreshActiveModel } from '../../../features/models/queries'
import { useAppStore } from '../../../app/store/useAppStore'
import { renderWithQuery } from '../../utils'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

const mockInvoke = vi.mocked(invoke)

const MODEL = {
  id: 'whisper-tiny',
  displayName: 'Whisper Tiny',
  description: 'Fast',
  detail: 'Fast',
  family: 'whisper',
  sizeBytes: 75_000_000,
  multilingual: true,
  pipelines: ['localAgreement'],
}

let onDisk = false

beforeEach(() => {
  onDisk = false
  mockInvoke.mockReset()
  mockInvoke.mockImplementation((cmd) => {
    switch (cmd) {
      case 'get_model_catalog': return Promise.resolve([MODEL])
      case 'get_hardware_profile':
        return Promise.resolve({ gpuName: 'GPU', executionProvider: 'cpu', vramGb: 0, recommendedModel: 'Whisper Tiny' })
      case 'get_model_info': return Promise.resolve({ modelName: 'Whisper Tiny', downloaded: onDisk, downloading: false })
      // The initial download list never arrives in these tests.
      case 'get_active_downloads': return new Promise(() => {})
      default: return Promise.resolve(null)
    }
  })
  useAppStore.setState({ modelChosen: false })
})

async function confirmDownload() {
  renderWithQuery(<ModelPickerModal />)
  const confirm = await screen.findByRole('button', { name: /download & continue/i })
  await waitFor(() => expect(confirm).toBeEnabled())
  fireEvent.click(confirm)
  await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('start_model_download', { id: 'whisper-tiny' }))
}

describe('ModelPickerModal', () => {
  it('stays open while the download list is still being read', async () => {
    await confirmDownload()
    await act(async () => { await Promise.resolve() })
    expect(useAppStore.getState().modelChosen).toBe(false)
  })

  it('closes once the backend reports the model on disk', async () => {
    await confirmDownload()
    onDisk = true
    await act(async () => { await refreshActiveModel() })
    await waitFor(() => expect(useAppStore.getState().modelChosen).toBe(true))
  })
})
