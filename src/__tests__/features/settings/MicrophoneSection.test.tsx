import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen, fireEvent, waitFor, act } from '@testing-library/react'
import { MicrophoneSection } from '../../../features/settings/MicrophoneSection'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { renderWithQuery } from '../../utils'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

/** Rust's saved device, which `list_input_devices` reports. */
let saved: string | null
/** Replies to `set_input_device`, in call order; unset ones save at once. */
let replies: Array<{ resolve: () => void; reject: (e: unknown) => void }>
let holdSaves: boolean
let deferred: string[]

const device = (id: string, name: string, isDefault = false) => ({ id, name, isDefault, isSelected: saved === id })

const devices = () => [
  device('realtek', 'Realtek Microphone', true),
  device('yeti', 'Blue Yeti'),
  device('headset', 'USB Headset'),
  device('usb-a', 'USB Mic'),
  device('usb-b', 'USB Mic'),
]

beforeEach(() => {
  saved = null
  replies = []
  holdSaves = false
  deferred = []
  vi.mocked(toast.error).mockClear()
  mockInvoke.mockReset()
  mockInvoke.mockImplementation((cmd, args) => {
    switch (cmd) {
      case 'list_input_devices': return Promise.resolve(devices())
      case 'get_recording_state': return Promise.resolve({ revision: 1, status: 'dictation', deferred })
      case 'get_keep_mic_open': return Promise.resolve(false)
      case 'set_input_device': {
        const id = (args as { id: string | null }).id
        if (!holdSaves) {
          saved = id
          return Promise.resolve(null)
        }
        return new Promise((resolve, reject) => {
          replies.push({ resolve: () => { saved = id; resolve(null) }, reject })
        })
      }
      default: return Promise.resolve(null)
    }
  })
})

const trigger = () => screen.getByRole('button', { name: 'Microphone' })

async function pick(name: string) {
  fireEvent.click(trigger())
  fireEvent.click(await screen.findByRole('option', { name }))
}

async function loaded() {
  renderWithQuery(<MicrophoneSection />)
  await waitFor(() => expect(trigger()).toHaveTextContent(/Default - Realtek Microphone/))
}

describe('MicrophoneSection', () => {
  it('preselects a saved non-default device', async () => {
    saved = 'yeti'
    renderWithQuery(<MicrophoneSection />)
    await waitFor(() => expect(trigger()).toHaveTextContent('Blue Yeti'))
  })

  it('saves the picked one of two identical mics by its id', async () => {
    await loaded()
    await pick('USB Mic (2)')
    await waitFor(() => expect(saved).toBe('usb-b'))
    expect(trigger()).toHaveTextContent('USB Mic (2)')
  })

  it('a failure of an older pick never undoes a newer one that saved', async () => {
    await loaded()
    holdSaves = true
    await pick('Blue Yeti')
    await pick('USB Headset')
    expect(trigger()).toHaveTextContent('USB Headset')

    // The newer pick waits for the older save, which fails.
    await act(async () => { replies[0].reject({ code: 'io', message: 'disk full' }) })
    expect(trigger()).toHaveTextContent('USB Headset')
    await waitFor(() => expect(replies).toHaveLength(2))
    await act(async () => { replies[1].resolve() })

    await waitFor(() => expect(trigger()).toHaveTextContent('USB Headset'))
    expect(saved).toBe('headset')
    expect(toast.error).not.toHaveBeenCalled()
  })

  it('shows what Rust kept when the newest pick fails', async () => {
    await loaded()
    holdSaves = true
    await pick('Blue Yeti')
    await pick('USB Headset')

    // The older pick saved; the newer one failed, so Rust holds the older.
    await act(async () => { replies[0].resolve() })
    await act(async () => { replies[1].reject({ code: 'io', message: 'disk full' }) })

    await waitFor(() => expect(trigger()).toHaveTextContent('Blue Yeti'))
    expect(toast.error).toHaveBeenCalledWith('disk full')
  })

  it('says a saved device waits while the recording in progress keeps the previous one', async () => {
    deferred = ['microphone']
    await loaded()
    expect(await screen.findByText('Saved. This recording keeps the previous one.')).toBeInTheDocument()
    expect(screen.queryByText('The device that records your voice.')).not.toBeInTheDocument()
  })
})
