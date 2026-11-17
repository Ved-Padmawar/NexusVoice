/**
 * First-run onboarding asks Rust to switch to the picked model once it is on
 * disk, and stays open until Rust reports that switch.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { act, fireEvent, screen, waitFor } from '@testing-library/react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { ModelPickerModal } from '../../../features/models/ModelPickerModal'
import { useAppStore } from '../../../app/store/useAppStore'
import { mirror, renderWithQuery, settingsAt, type Send } from '../../utils'

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

const mockInvoke = vi.mocked(invoke)

const model = (id: string, displayName: string) => ({
  id,
  displayName,
  description: 'Fast',
  detail: 'Fast',
  family: 'whisper',
  sizeBytes: 75_000_000,
  multilingual: true,
  pipelines: ['localAgreement'],
})

let handlers: Map<string, (e: { payload: unknown }) => void>
let send: Send
let rev = 0

/** Rust's download list landed with the pick queued, so its start settles. */
const queued = () => send({ slice: 'downloads', rev: 1, value: [{ id: 'whisper-tiny', status: 'queued', progress: 0, error: null }] })
/** Rust made `id` the active model, with `onDisk` the models on disk. */
const activeIs = async (id: string, onDisk: string[]) => {
  await send({ slice: 'models', rev: ++rev, value: onDisk.map(variant => ({ variant, displayName: variant, sizeBytes: 1 })) })
  await send(settingsAt(++rev, { activeModel: id }))
}

beforeEach(async () => {
  rev = 0
  handlers = new Map()
  vi.mocked(listen).mockImplementation(async (event, handler) => {
    handlers.set(event, handler as (e: { payload: unknown }) => void)
    return () => {}
  })
  mockInvoke.mockReset()
  mockInvoke.mockImplementation((cmd) => {
    switch (cmd) {
      case 'get_model_catalog': return Promise.resolve([model('whisper-tiny', 'Whisper Tiny'), model('parakeet', 'Parakeet')])
      case 'get_hardware_profile':
        return Promise.resolve({ gpuName: 'GPU', executionProvider: 'cpu', vramGb: 0, recommendedModel: 'Whisper Tiny' })
      case 'start_model_download': return Promise.resolve({ rev: 1 })
      default: return Promise.resolve(null)
    }
  })
  useAppStore.setState({ modelChosen: false })
  // Another model is active, and nothing is on disk yet.
  send = await mirror([settingsAt(0, { activeModel: 'parakeet' })])
})

async function confirmDownload() {
  renderWithQuery(<ModelPickerModal />)
  const confirm = await screen.findByRole('button', { name: /download & continue/i })
  await waitFor(() => expect(confirm).toBeEnabled())
  fireEvent.click(confirm)
  await waitFor(() =>
    expect(mockInvoke).toHaveBeenCalledWith('start_model_download', { id: 'whisper-tiny', activate: true }))
}

describe('ModelPickerModal', () => {
  it('leaves the active model alone until the download lands', async () => {
    await confirmDownload()
    await act(async () => { await Promise.resolve() })
    expect(mockInvoke).not.toHaveBeenCalledWith('set_model_override', expect.anything())
    expect(useAppStore.getState().modelChosen).toBe(false)
  })

  it('stays open while another model is the one on disk', async () => {
    await confirmDownload()
    await queued()
    await activeIs('parakeet', ['parakeet'])
    expect(useAppStore.getState().modelChosen).toBe(false)
  })

  it('stays on the model list with an error when the download cannot start', async () => {
    const base = mockInvoke.getMockImplementation()!
    mockInvoke.mockImplementation((cmd, args) =>
      cmd === 'start_model_download' ? Promise.reject({ code: 'io', message: 'no' }) : base(cmd, args))
    renderWithQuery(<ModelPickerModal />)
    const confirm = await screen.findByRole('button', { name: /download & continue/i })
    await waitFor(() => expect(confirm).toBeEnabled())
    fireEvent.click(confirm)

    expect(await screen.findByText('Could not start the download.')).toBeInTheDocument()
    expect(screen.queryByText(/download complete/i)).not.toBeInTheDocument()
    await waitFor(() => expect(screen.getByRole('button', { name: /download & continue/i })).toBeEnabled())
  })

  it('keeps a hand pick when the hardware recommendation arrives later', async () => {
    let recommend!: (profile: unknown) => void
    const base = mockInvoke.getMockImplementation()!
    mockInvoke.mockImplementation((cmd, args) =>
      cmd === 'get_hardware_profile' ? new Promise(resolve => { recommend = resolve }) : base(cmd, args))
    renderWithQuery(<ModelPickerModal />)
    fireEvent.click(await screen.findByRole('button', { name: /parakeet/i }))
    await act(async () => {
      recommend({ gpuName: 'GPU', executionProvider: 'cpu', vramGb: 0, recommendedModel: 'Whisper Tiny' })
    })

    fireEvent.click(screen.getByRole('button', { name: /download & continue/i }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('start_model_download', { id: 'parakeet', activate: true }))
  })

  it('returns to the list when the download is cancelled, even while it is still starting', async () => {
    // The start waits for the download list, which lands only after the cancel.
    await confirmDownload()
    await act(async () => { handlers.get('model-download-cancelled')?.({ payload: { id: 'whisper-tiny' } }) })
    await send({ slice: 'downloads', rev: 1, value: [] })

    expect(await screen.findByRole('button', { name: /download & continue/i })).toBeEnabled()
    expect(useAppStore.getState().modelChosen).toBe(false)
  })

  it('keeps the confirmed pick when another model is clicked while its download starts', async () => {
    await confirmDownload()
    fireEvent.click(screen.getByRole('button', { name: /parakeet/i }))
    await queued()

    await activeIs('whisper-tiny', ['whisper-tiny'])
    await waitFor(() => expect(useAppStore.getState().modelChosen).toBe(true))
  })

  it('closes once Rust reports the pick active and on disk', async () => {
    await confirmDownload()
    await queued()
    await activeIs('whisper-tiny', ['whisper-tiny'])
    await waitFor(() => expect(useAppStore.getState().modelChosen).toBe(true))
  })
})
