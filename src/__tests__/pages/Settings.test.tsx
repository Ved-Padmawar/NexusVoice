import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { Settings } from '../../pages/Settings'
import { useAppStore } from '../../store/useAppStore'
import { invoke } from '@tauri-apps/api/core'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

// The Recording (push-to-talk) row's recorder — uniquely identified by its aria-label.
const recordingRecorder = () => screen.getByRole('button', { name: /click to record recording hotkey/i })

// HotkeySection lives in the "Shortcuts" tab; the tab is controlled by the store.
function renderSettings() {
  useAppStore.setState({ activeSettingsTab: 'shortcuts' })
  return render(
    <MemoryRouter>
      <Settings />
    </MemoryRouter>
  )
}

beforeEach(() => {
  mockInvoke.mockReset()
  mockInvoke.mockResolvedValue(undefined)
  useAppStore.setState({ hasHotkey: false, hasDictationHotkey: false, hasDictationCommitHotkey: false })
})

describe('HotkeySection', () => {
  it('shows click-to-set placeholder when no hotkey set', () => {
    renderSettings()
    // One placeholder per unset row (3 rows).
    expect(screen.getAllByText(/click to set/i).length).toBeGreaterThan(0)
  })

  it('enters listening mode on click', () => {
    renderSettings()
    fireEvent.click(recordingRecorder())
    expect(screen.getByText(/press keys/i)).toBeInTheDocument()
  })

  it('shows pressed keys while listening', () => {
    renderSettings()
    fireEvent.click(recordingRecorder())
    fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
    fireEvent.keyDown(window, { key: 'A', code: 'KeyA' })
    expect(screen.getByText('Ctrl')).toBeInTheDocument()
    expect(screen.getByText('A')).toBeInTheDocument()
  })

  it('shows Save and Cancel buttons after keys pressed', () => {
    renderSettings()
    fireEvent.click(recordingRecorder())
    fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
    fireEvent.keyDown(window, { key: 'A', code: 'KeyA' })
    expect(screen.getByRole('button', { name: /save/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /cancel/i })).toBeInTheDocument()
  })

  it('calls register_hotkey and shows current hotkey on save', async () => {
    mockInvoke.mockResolvedValue(undefined)
    renderSettings()
    fireEvent.click(recordingRecorder())
    fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
    fireEvent.keyDown(window, { key: 'A', code: 'KeyA' })
    fireEvent.click(screen.getByRole('button', { name: /save/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('register_hotkey', { hotkey: 'Ctrl+A' })
    })
    expect(useAppStore.getState().hasHotkey).toBe(true)
  })

  it('cancels and clears pressed keys', () => {
    renderSettings()
    fireEvent.click(recordingRecorder())
    fireEvent.keyDown(window, { key: 'Control', code: 'ControlLeft' })
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }))
    expect(screen.getAllByText(/click to set/i).length).toBeGreaterThan(0)
  })

  it('loads existing hotkey on mount when hasHotkey is true', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_registered_hotkeys') return Promise.resolve({ ptt: ['Ctrl+Space'], dictation: [], dictation_commit: [] })
      return Promise.resolve(undefined)
    })
    useAppStore.setState({ hasHotkey: true })
    renderSettings()
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_registered_hotkeys')
    })
  })

  it('calls unregister_hotkey and clears hotkey on remove', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_registered_hotkeys') return Promise.resolve({ ptt: ['Ctrl+A'], dictation: [], dictation_commit: [] })
      return Promise.resolve(undefined)
    })
    useAppStore.setState({ hasHotkey: true })
    renderSettings()
    const removeBtn = await screen.findByRole('button', { name: /remove recording hotkey/i })
    fireEvent.click(removeBtn)
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('unregister_hotkey')
    })
    expect(useAppStore.getState().hasHotkey).toBe(false)
  })
})
