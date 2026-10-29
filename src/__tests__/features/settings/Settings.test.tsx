import { describe, it, expect, vi, beforeEach } from 'vitest'
import { act, render, screen, fireEvent, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { Settings } from '../../../features/settings/Settings'
import { useAppStore } from '../../../app/store/useAppStore'
import { invoke } from '@tauri-apps/api/core'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

/** What the General tab's sections load on mount, plus the saved hotkeys. */
const respond = (hotkeys = { pushToTalk: null as string | null, dictation: null, dictationCommit: null }) =>
  mockInvoke.mockImplementation((cmd) => Promise.resolve(({
    get_hotkeys: hotkeys,
    list_input_devices: [],
    get_keep_mic_open: false,
    get_language_options: { supported: false, options: [] },
    get_format_config: { enabled: false, provider: 'ollama', profiles: {} },
  } as Record<string, unknown>)[cmd]))

// The Recording (push-to-talk) row's recorder — uniquely identified by its aria-label.
const recordingRecorder = () => screen.getByRole('button', { name: /click to record recording hotkey/i })

// HotkeySection lives in the "General" tab; the tab is controlled by the store.
// The tab also mounts the microphone and language sections, which fetch on
// mount — await here so their state lands inside act() rather than after the
// test has finished.
async function renderSettings() {
  useAppStore.setState({ activeSettingsTab: 'general' })
  const result = render(
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
  useAppStore.setState({ hasHotkey: false, hasDictationHotkey: false, hasDictationCommitHotkey: false })
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

  it('shows Save and Cancel buttons after keys pressed', async () => {
    await renderSettings()
    fireEvent.click(recordingRecorder())
    fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
    fireEvent.keyDown(window, { key: 'A', code: 'KeyA' })
    expect(screen.getByRole('button', { name: /save/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /cancel/i })).toBeInTheDocument()
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
    expect(useAppStore.getState().hasHotkey).toBe(true)
  })

  it('cancels and clears pressed keys', async () => {
    await renderSettings()
    fireEvent.click(recordingRecorder())
    fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }))
    expect(screen.getAllByText(/click to set/i).length).toBeGreaterThan(0)
  })

  it('loads existing hotkey on mount when hasHotkey is true', async () => {
    respond({ pushToTalk: 'Ctrl+Space', dictation: null, dictationCommit: null })
    useAppStore.setState({ hasHotkey: true })
    await renderSettings()
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_hotkeys')
    })
  })

  it('puts a saved hotkey back when the page closes mid-edit', async () => {
    respond({ pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null })
    useAppStore.setState({ hasHotkey: true })
    const { unmount } = await renderSettings()
    fireEvent.click(await screen.findByRole('button', { name: /change recording hotkey/i }))
    expect(mockInvoke).toHaveBeenCalledWith('set_hotkey', { kind: 'pushToTalk', hotkey: null })

    unmount()

    expect(mockInvoke).toHaveBeenLastCalledWith('set_hotkey', { kind: 'pushToTalk', hotkey: 'Ctrl+A' })
  })

  it('does not restore the old hotkey after a new one is saved', async () => {
    respond({ pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null })
    useAppStore.setState({ hasHotkey: true })
    const { unmount } = await renderSettings()
    fireEvent.click(await screen.findByRole('button', { name: /change recording hotkey/i }))
    fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
    fireEvent.keyDown(window, { key: 'B', code: 'KeyB' })
    fireEvent.click(screen.getByRole('button', { name: /save/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_hotkey', { kind: 'pushToTalk', hotkey: 'Ctrl+B' })
    })

    unmount()

    expect(mockInvoke).not.toHaveBeenCalledWith('set_hotkey', { kind: 'pushToTalk', hotkey: 'Ctrl+A' })
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
      useAppStore.setState({ hasHotkey: true })
      const { unmount } = await renderSettings()
      fireEvent.click(await screen.findByRole('button', { name: /change recording hotkey/i }))
      fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
      fireEvent.keyDown(window, { key: 'B', code: 'KeyB' })
      fireEvent.click(screen.getByRole('button', { name: /save/i }))
      unmount()
    })

    it('does not restore the old hotkey over it', () => {
      expect(mockInvoke).not.toHaveBeenCalledWith('set_hotkey', { kind: 'pushToTalk', hotkey: 'Ctrl+A' })
    })

    it('restores the old hotkey if the save fails', async () => {
      await act(async () => { rejectSave({ code: 'hotkey', message: 'in use' }) })
      expect(mockInvoke).toHaveBeenLastCalledWith('set_hotkey', { kind: 'pushToTalk', hotkey: 'Ctrl+A' })
    })
  })

  it('unbinds the hotkey on remove', async () => {
    respond({ pushToTalk: 'Ctrl+A', dictation: null, dictationCommit: null })
    useAppStore.setState({ hasHotkey: true })
    await renderSettings()
    const removeBtn = await screen.findByRole('button', { name: /remove recording hotkey/i })
    fireEvent.click(removeBtn)
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_hotkey', { kind: 'pushToTalk', hotkey: null })
    })
    expect(useAppStore.getState().hasHotkey).toBe(false)
  })
})
