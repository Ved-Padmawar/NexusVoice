import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen, fireEvent, waitFor, act } from '@testing-library/react'
import { LanguageSection } from '../../../features/settings/LanguageSection'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { renderWithQuery } from '../../utils'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

const NAMES: Record<string, string> = { auto: 'Auto-detect', 'en-US': 'English (US)', 'fr-FR': 'French (FR)', de: 'German' }

let saved: string
let replies: Array<{ resolve: () => void; reject: (e: unknown) => void }>
let deferred: string[]

beforeEach(() => {
  saved = 'en-US'
  replies = []
  deferred = []
  vi.mocked(toast.error).mockClear()
  mockInvoke.mockReset()
  mockInvoke.mockImplementation((cmd, args) => {
    switch (cmd) {
      case 'get_language_options':
        return Promise.resolve({
          supported: true,
          options: Object.entries(NAMES).map(([code, name]) => ({ code, name, isSelected: code === saved })),
        })
      case 'get_recording_state': return Promise.resolve({ revision: 1, status: 'dictation', deferred })
      case 'set_language': {
        const code = (args as { code: string }).code
        return new Promise((resolve, reject) => {
          replies.push({ resolve: () => { saved = code; resolve(null) }, reject })
        })
      }
      default: return Promise.resolve(null)
    }
  })
})

const trigger = () => screen.getByRole('button', { name: 'Language' })

async function pick(name: string) {
  fireEvent.click(trigger())
  fireEvent.click(await screen.findByRole('option', { name }))
}

describe('LanguageSection', () => {
  it('a failure of an older pick never undoes a newer one that saved', async () => {
    renderWithQuery(<LanguageSection />)
    await waitFor(() => expect(trigger()).toHaveTextContent('English (US)'))

    await pick('French (FR)')
    await pick('German')
    await act(async () => { replies[0].reject({ code: 'io', message: 'could not write settings.json' }) })
    await waitFor(() => expect(replies).toHaveLength(2))
    await act(async () => { replies[1].resolve() })

    await waitFor(() => expect(trigger()).toHaveTextContent('German'))
    expect(saved).toBe('de')
    expect(toast.error).not.toHaveBeenCalled()
  })

  it('shows Rust\'s language once a failed pick settles', async () => {
    renderWithQuery(<LanguageSection />)
    await waitFor(() => expect(trigger()).toHaveTextContent('English (US)'))

    await pick('German')
    expect(trigger()).toHaveTextContent('German')
    await act(async () => { replies[0].reject({ code: 'invalid_language', message: "This model doesn't support that language." }) })

    await waitFor(() => expect(trigger()).toHaveTextContent('English (US)'))
    expect(toast.error).toHaveBeenCalledWith("This model doesn't support that language.")
  })

  it('says a saved language waits while the recording in progress keeps the previous one', async () => {
    deferred = ['language']
    renderWithQuery(<LanguageSection />)
    await waitFor(() => expect(trigger().parentElement).toHaveAttribute('data-tip', 'Saved. This recording keeps the previous one.'))
  })

  it('is disabled and says why for an English-only model', async () => {
    mockInvoke.mockImplementation(cmd =>
      Promise.resolve(cmd === 'get_language_options' ? { supported: false, options: [] } : null))
    renderWithQuery(<LanguageSection />)
    await waitFor(() => expect(trigger().parentElement).toHaveAttribute('data-tip', 'This model only understands English'))
    expect(trigger()).toBeDisabled()
  })
})
