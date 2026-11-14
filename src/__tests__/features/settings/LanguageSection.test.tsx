import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen } from '@testing-library/react'
import { LanguageSection } from '../../../features/settings/LanguageSection'
import { invoke } from '@tauri-apps/api/core'
import { mirror, renderWithQuery, settingsAt } from '../../utils'

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

const FRENCH = { supported: true, options: [{ code: 'auto', name: 'Auto-detect', isSelected: false }, { code: 'fr-FR', name: 'French (FR)', isSelected: true }] }

beforeEach(() => {
  mockInvoke.mockReset().mockResolvedValue(null)
})

const trigger = () => screen.getByRole('button', { name: 'Language' })

describe('LanguageSection', () => {
  it('says a saved language waits while the recording in progress keeps the previous one', async () => {
    await mirror([settingsAt(0, { language: FRENCH }), { slice: 'recording', rev: 0, value: { status: 'dictation', deferred: ['language'] } }])
    renderWithQuery(<LanguageSection />)
    expect(trigger()).toHaveTextContent('French (FR)')
    expect(trigger().parentElement).toHaveAttribute('data-tip', 'Saved. This recording keeps the previous one.')
  })

  it('is disabled and says why for an English-only model', async () => {
    await mirror([settingsAt(0, { language: { supported: false, options: [] } })])
    renderWithQuery(<LanguageSection />)
    expect(trigger().parentElement).toHaveAttribute('data-tip', 'This model only understands English')
    expect(trigger()).toBeDisabled()
  })
})
