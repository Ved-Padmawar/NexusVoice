import { describe, it, expect, vi, beforeEach } from 'vitest'
import { act, screen, fireEvent, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { Settings } from '../../../features/settings/Settings'
import { useAppStore } from '../../../app/store/useAppStore'
import { latest } from '../../../app/mirror'
import type { HotkeyBindings } from '../../../shared/types'
import { invoke } from '@tauri-apps/api/core'
import { mirror, renderWithQuery, settingsAt } from '../../utils'

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

const NO_HOTKEYS: HotkeyBindings = { pushToTalk: null, dictation: null, dictationCommit: null }

/** What the General tab's sections load on mount, and the saved hotkeys,
 *  which `set_hotkey` changes and sends as Rust would. `granted` are the ones
 *  the OS bound. */
async function respond(initial = NO_HOTKEYS, granted = initial) {
  let hotkeys = initial
  let rev = 0
  mockInvoke.mockImplementation((cmd, args) => {
    if (cmd === 'set_hotkey') {
      const { kind, hotkey } = args as { kind: keyof HotkeyBindings; hotkey: string | null }
      hotkeys = { ...hotkeys, [kind]: hotkey }
      rev += 1
      setTimeout(() => { void send(settingsAt(rev, { hotkeys })) })
      return Promise.resolve({ rev })
    }
    return Promise.resolve(({
      get_model_catalog: [],
      list_input_devices: [],
      get_recording_state: { revision: 0, status: 'idle', deferred: [] },
      get_jobs: [],
    } as Record<string, unknown>)[cmd])
  })
  const send = await mirror([settingsAt(0, { hotkeys }), { slice: 'hotkeys', rev: 0, value: granted }])
}

// The Recording (push-to-talk) row's recorder — uniquely identified by its aria-label.
const recordingRecorder = () => screen.getByRole('button', { name: /click to record recording hotkey/i })

const savedPushToTalk = () => latest('settings').hotkeys.pushToTalk

// HotkeySection lives in the "General" tab; the tab is controlled by the store.
// The tab also mounts the microphone and language sections, which fetch on
// mount — await here so their state lands inside act() rather than after the
// test has finished.
async function renderSettings() {
  useAppStore.setState({ activeSettingsTab: 'general' })
  const result = renderWithQuery(
    <MemoryRouter>
      <Settings />
    </MemoryRouter>
  )
  await act(async () => { await Promise.resolve() })
  return result
}

beforeEach(async () => {
  mockInvoke.mockReset()
  await respond()
})

describe('HotkeySection', () => {
  it('shows click-to-set placeholder when no hotkey set', async () => {
    await renderSettings()
    // One placeholder per unset row (3 rows).
    expect(screen.getAllByText(/click to set/i).length).toBeGreaterThan(0)
  })

  it('enters listening mode on click', async () => {
    await renderSettings()
    fireEvent.click(recordingRecorder())
    expect(screen.getByText(/press the keys/i)).toBeInTheDocument()
  })

  it('shows pressed keys while listening', async () => {
    await renderSettings()
    fireEvent.click(recordingRecorder())
    fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
    fireEvent.keyDown(window, { key: 'A', code: 'KeyA' })
    expect(screen.getByText('Ctrl')).toBeInTheDocument()
    expect(screen.getByText('A')).toBeInTheDocument()
  })

  it('binds the recorded hotkey on save', async () => {
    await renderSettings()
    fireEvent.click(recordingRecorder())
    fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
    fireEvent.keyDown(window, { key: 'A', code: 'KeyA' })
    fireEvent.click(screen.getByRole('button', { name: /save/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_hotkey', { kind: 'pushToTalk', hotkey: 'Ctrl+A' })
    })
    expect(savedPushToTalk()).toBe('Ctrl+A')
  })

  it('keeps the Windows key as a modifier', async () => {
    await renderSettings()
    fireEvent.click(recordingRecorder())
    fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
    fireEvent.keyDown(window, { key: 'Meta', code: 'MetaLeft' })
    fireEvent.keyDown(window, { key: 'K', code: 'KeyK' })
    fireEvent.click(screen.getByRole('button', { name: /save/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_hotkey', { kind: 'pushToTalk', hotkey: 'Ctrl+Super+K' })
    })
  })

  it('names keys by position, so Shift+1 is saved as Shift+1 rather than Shift+!', async () => {
    await renderSettings()
    fireEvent.click(recordingRecorder())
    fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
    fireEvent.keyDown(window, { key: 'Shift', code: 'ShiftLeft' })
    fireEvent.keyDown(window, { key: '!', code: 'Digit1' })
    fireEvent.click(screen.getByRole('button', { name: /save/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_hotkey', { kind: 'pushToTalk', hotkey: 'Ctrl+Shift+1' })
    })
  })

  it('cancels on Escape instead of recording it', async () => {
    await renderSettings()
    fireEvent.click(recordingRecorder())
    fireEvent.keyDown(window, { key: 'Escape', code: 'Escape' })
    expect(screen.queryByText('Esc')).not.toBeInTheDocument()
    expect(mockInvoke).toHaveBeenCalledWith('resume_hotkeys')
  })

  it('cancels and clears pressed keys', async () => {
    await renderSettings()
    fireEvent.click(recordingRecorder())
    fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }))
    expect(screen.getAllByText(/click to set/i).length).toBeGreaterThan(0)
  })

  it('binds the hotkeys again when editing is cancelled', async () => {
    await renderSettings()
    fireEvent.click(recordingRecorder())
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }))
    expect(mockInvoke).toHaveBeenCalledWith('resume_hotkeys')
  })

  it('shows the saved hotkey on mount', async () => {
    await respond({ pushToTalk: 'Ctrl+Space', dictation: null, dictationCommit: null })
    await renderSettings()
    expect(await screen.findByText('Space')).toBeInTheDocument()
  })

  it('warns when a saved hotkey is not the one the OS granted', async () => {
    const saved = { pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null }
    await respond(saved, { ...saved, pushToTalk: null })
    await renderSettings()
    expect(await screen.findByText(/another app is using this shortcut/i)).toBeInTheDocument()
  })

  it('releases the hotkeys while editing without changing the saved one', async () => {
    await respond({ pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null })
    await renderSettings()
    fireEvent.click(await screen.findByRole('button', { name: /change recording hotkey/i }))

    expect(mockInvoke).toHaveBeenCalledWith('suspend_hotkeys')
    expect(mockInvoke).not.toHaveBeenCalledWith('set_hotkey', expect.anything())
  })

  it('binds the hotkeys again when the page closes mid-edit', async () => {
    await respond({ pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null })
    const { unmount } = await renderSettings()
    fireEvent.click(await screen.findByRole('button', { name: /change recording hotkey/i }))

    unmount()

    expect(mockInvoke).toHaveBeenCalledWith('resume_hotkeys')
  })

  it('does not resume the hotkeys after a new one is saved', async () => {
    await respond({ pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null })
    const { unmount } = await renderSettings()
    fireEvent.click(await screen.findByRole('button', { name: /change recording hotkey/i }))
    fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
    fireEvent.keyDown(window, { key: 'B', code: 'KeyB' })
    fireEvent.click(screen.getByRole('button', { name: /save/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_hotkey', { kind: 'pushToTalk', hotkey: 'Ctrl+B' })
    })

    unmount()

    expect(mockInvoke).not.toHaveBeenCalledWith('resume_hotkeys')
  })

  it('binds the hotkeys again right away when a save fails', async () => {
    await respond({ pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null })
    const base = mockInvoke.getMockImplementation()!
    mockInvoke.mockImplementation((cmd, args) =>
      cmd === 'set_hotkey' ? Promise.reject({ code: 'hotkey', message: 'in use' }) : base(cmd, args))
    await renderSettings()
    fireEvent.click(await screen.findByRole('button', { name: /change recording hotkey/i }))
    fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
    fireEvent.keyDown(window, { key: 'B', code: 'KeyB' })
    fireEvent.keyUp(window, { key: 'B', code: 'KeyB' })
    await waitFor(() => expect(screen.queryByText(/press the keys/i)).not.toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /save/i }))

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('resume_hotkeys'))
    await waitFor(() => expect(screen.queryByRole('button', { name: /save/i })).not.toBeInTheDocument())
  })

  describe('closing the page while a save is pending', () => {
    let rejectSave: (e: unknown) => void

    beforeEach(async () => {
      await respond({ pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null })
      const base = mockInvoke.getMockImplementation()!
      mockInvoke.mockImplementation((cmd, args) =>
        cmd === 'set_hotkey' && (args as { hotkey: string | null }).hotkey === 'Ctrl+B'
          ? new Promise((_, reject) => { rejectSave = reject })
          : base(cmd, args))
      const { unmount } = await renderSettings()
      fireEvent.click(await screen.findByRole('button', { name: /change recording hotkey/i }))
      fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
      fireEvent.keyDown(window, { key: 'B', code: 'KeyB' })
      fireEvent.click(screen.getByRole('button', { name: /save/i }))
      unmount()
    })

    it('does not resume the hotkeys over it', () => {
      expect(mockInvoke).not.toHaveBeenCalledWith('resume_hotkeys')
    })

    it('resumes the hotkeys if the save fails', async () => {
      await act(async () => { rejectSave({ code: 'hotkey', message: 'in use' }) })
      expect(mockInvoke).toHaveBeenCalledWith('resume_hotkeys')
    })
  })

  it('unbinds the hotkey on remove', async () => {
    await respond({ pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null })
    await renderSettings()
    const removeBtn = await screen.findByRole('button', { name: /remove recording hotkey/i })
    await waitFor(() => expect(removeBtn).toBeEnabled())
    fireEvent.click(removeBtn)
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_hotkey', { kind: 'pushToTalk', hotkey: null })
    })
    await waitFor(() => expect(savedPushToTalk()).toBeNull())
  })
})
