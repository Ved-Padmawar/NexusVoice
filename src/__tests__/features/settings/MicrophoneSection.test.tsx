import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen, fireEvent, waitFor } from '@testing-library/react'
import { MicrophoneSection } from '../../../features/settings/MicrophoneSection'
import { invoke } from '@tauri-apps/api/core'
import { mirror, renderWithQuery, settingsAt } from '../../utils'

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

let saves: (string | null)[]
let deferred: string[]

const device = (id: string, name: string, isDefault = false) => ({ id, name, isDefault })

const DEVICES = [
  device('realtek', 'Realtek Microphone', true),
  device('yeti', 'Blue Yeti'),
  device('headset', 'USB Headset'),
  device('usb-a', 'USB Mic'),
  device('usb-b', 'USB Mic'),
]

/** Render with `saved` as Rust's saved device. */
async function renderSaved(saved: string | null = null) {
  await mirror([settingsAt(0, { inputDevice: saved })])
  renderWithQuery(<MicrophoneSection />)
  await waitFor(() => expect(trigger()).toBeEnabled())
}

beforeEach(() => {
  saves = []
  deferred = []
  mockInvoke.mockReset()
  mockInvoke.mockImplementation((cmd, args) => {
    switch (cmd) {
      case 'list_input_devices': return Promise.resolve(DEVICES)
      case 'get_recording_state': return Promise.resolve({ revision: 1, status: 'dictation', deferred })
      case 'set_input_device':
        saves.push((args as { id: string | null }).id)
        return Promise.resolve({ rev: 0 })
      default: return Promise.resolve(null)
    }
  })
})

const trigger = () => screen.getByRole('button', { name: 'Microphone' })

describe('MicrophoneSection', () => {
  it('preselects a saved non-default device', async () => {
    await renderSaved('yeti')
    expect(trigger()).toHaveTextContent('Blue Yeti')
  })

  it('saves the picked one of two identical mics by its id', async () => {
    await renderSaved()
    fireEvent.click(trigger())
    fireEvent.click(await screen.findByRole('option', { name: 'USB Mic (2)' }))
    await waitFor(() => expect(saves).toEqual(['usb-b']))
  })

  it('says a saved device waits while the recording in progress keeps the previous one', async () => {
    deferred = ['microphone']
    await renderSaved()
    expect(await screen.findByText('Saved. This recording keeps the previous one.')).toBeInTheDocument()
    expect(screen.queryByText('The device that records your voice.')).not.toBeInTheDocument()
  })
})
