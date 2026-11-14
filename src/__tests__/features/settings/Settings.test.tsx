import { describe, it, expect, vi, beforeEach } from 'vitest'
import { act, screen, fireEvent, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { Settings } from '../../../features/settings/Settings'
import { useAppStore } from '../../../app/store/useAppStore'
import { queryClient } from '../../../app/queryClient'
import { queryKeys } from '../../../shared/queryKeys'
import type { HotkeyBindings } from '../../../shared/types'
import { invoke } from '@tauri-apps/api/core'
import { renderWithQuery } from '../../utils'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

/** What the General tab's sections load on mount, plus the saved hotkeys,
 *  which `set_hotkey` changes as Rust would. */
const respond = (initial: HotkeyBindings = { pushToTalk: null, dictation: null, dictationCommit: null }) => {
  let hotkeys = initial
  mockInvoke.mockImplementation((cmd, args) => {
    if (cmd === 'set_hotkey') {
      const { kind, hotkey } = args as { kind: keyof HotkeyBindings; hotkey: string | null }
      hotkeys = { ...hotkeys, [kind]: hotkey }
      return Promise.resolve(null)
    }
    return Promise.resolve(({
    get_hotkeys: hotkeys,
    get_bound_hotkeys: hotkeys,
    get_model_catalog: [],
    get_model_info: { modelName: 'Whisper Tiny', downloaded: true },
    list_input_devices: [],
    get_keep_mic_open: false,
    get_language_options: { supported: false, options: [] },
    get_format_config: { enabled: false, provider: 'ollama', profiles: {} },
    get_recording_state: { revision: 0, status: 'idle', deferred: [] },
    get_model_unload: 'never',
    get_paste_mode: 'auto',
    get_saved_audio: { enabled: true, keep: 10, bytes: 0 },
    get_jobs: [],
    get_pill_settings: { theme: 'steel', waveformStyle: 'bars', liveTranscript: false, startSound: false },
  } as Record<string, unknown>)[cmd])
  })
}

// The Recording (push-to-talk) row's recorder — uniquely identified by its aria-label.
const recordingRecorder = () => screen.getByRole('button', { name: /click to record recording hotkey/i })

const cachedPushToTalk = () => queryClient.getQueryData<HotkeyBindings>(queryKeys.hotkeys)?.pushToTalk

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

beforeEach(() => {
  mockInvoke.mockReset()
  respond()
})

describe('FormattingToggle', () => {
  it('shows the saved setting again when turning it off fails to save', async () => {
    const saved = {
      enabled: true,
      provider: 'ollama',
      profiles: { ollama: { baseUrl: 'http://localhost:11434/v1', model: 'qwen2.5-3b-instruct', hasApiKey: false } },
    }
    const base = mockInvoke.getMockImplementation()!
    mockInvoke.mockImplementation((cmd, args) => {
      if (cmd === 'get_format_config') return Promise.resolve(saved)
      if (cmd === 'set_format_config') return Promise.reject({ code: 'io', message: 'disk full' })
      return base(cmd, args)
    })
    await renderSettings()
    const toggle = await screen.findByRole('switch', { name: /smart formatting/i })
    await waitFor(() => expect(toggle).toBeChecked())

    fireEvent.click(toggle)

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('set_format_config', expect.anything()))
    await waitFor(() => expect(toggle).toBeChecked())
  })
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
    expect(cachedPushToTalk()).toBe('Ctrl+A')
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
    respond({ pushToTalk: 'Ctrl+Space', dictation: null, dictationCommit: null })
    await renderSettings()
    expect(await screen.findByText('Space')).toBeInTheDocument()
  })

  it('warns when a saved hotkey is not the one the OS granted', async () => {
    const saved = { pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null }
    const base = mockInvoke.getMockImplementation()!
    mockInvoke.mockImplementation((cmd, args) =>
      cmd === 'get_hotkeys' ? Promise.resolve(saved)
        : cmd === 'get_bound_hotkeys' ? Promise.resolve({ ...saved, pushToTalk: null })
          : base(cmd, args))
    await renderSettings()
    expect(await screen.findByText(/another app is using this shortcut/i)).toBeInTheDocument()
  })

  it('releases the hotkeys while editing without changing the saved one', async () => {
    respond({ pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null })
    await renderSettings()
    fireEvent.click(await screen.findByRole('button', { name: /change recording hotkey/i }))

    expect(mockInvoke).toHaveBeenCalledWith('suspend_hotkeys')
    expect(mockInvoke).not.toHaveBeenCalledWith('set_hotkey', expect.anything())
  })

  it('binds the hotkeys again when the page closes mid-edit', async () => {
    respond({ pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null })
    const { unmount } = await renderSettings()
    fireEvent.click(await screen.findByRole('button', { name: /change recording hotkey/i }))

    unmount()

    expect(mockInvoke).toHaveBeenCalledWith('resume_hotkeys')
  })

  it('does not resume the hotkeys after a new one is saved', async () => {
    respond({ pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null })
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
    respond({ pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null })
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
      respond({ pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null })
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
    respond({ pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null })
    await renderSettings()
    const removeBtn = await screen.findByRole('button', { name: /remove recording hotkey/i })
    await waitFor(() => expect(removeBtn).toBeEnabled())
    fireEvent.click(removeBtn)
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_hotkey', { kind: 'pushToTalk', hotkey: null })
    })
    await waitFor(() => expect(cachedPushToTalk()).toBeNull())
  })
})
