import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen, fireEvent, waitFor, act } from '@testing-library/react'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { FormattingToggle } from '../../../features/settings/FormattingToggle'
import type { FormatterView } from '../../../shared/bindings'
import { mirror, renderWithQuery, settingsAt } from '../../utils'

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)
const CONFIG: FormatterView = { enabled: false, provider: 'custom', profiles: { custom: { baseUrl: 'http://127.0.0.1:1/v1', model: 'good', hasApiKey: false } } }

let answerTest: (() => void) | null = null
let failTest: (() => void) | null = null

beforeEach(async () => {
  vi.mocked(toast.success).mockClear()
  vi.mocked(toast.info).mockClear()
  vi.mocked(toast.error).mockClear()
  mockInvoke.mockReset().mockImplementation((cmd) => {
    if (cmd === 'test_format_connection') {
      return new Promise<void>((resolve, reject) => {
        answerTest = resolve
        failTest = () => reject({ code: 'llm', message: 'model not found' })
      })
    }
    return Promise.resolve(null)
  })
  await mirror([settingsAt(0, { formatter: CONFIG })])
})

async function openForm() {
  renderWithQuery(<FormattingToggle />)
  await waitFor(() => expect(screen.getByText('good')).toBeInTheDocument())
  fireEvent.click(screen.getByRole('button', { name: /configure/i }))
  return await screen.findByRole('textbox', { name: /model/i })
}

const testButton = () => screen.getByRole('button', { name: /test connection/i })

describe('FormattingToggle connection test', () => {
  it('reports a result for the draft that was tested', async () => {
    await openForm()
    fireEvent.click(testButton())
    await act(async () => { answerTest!() })
    await waitFor(() => expect(toast.success).toHaveBeenCalledWith('Connection successful'))
    expect(testButton().querySelector('.text-success')).not.toBeNull()
  })

  it('does not credit a success to settings edited while the test ran', async () => {
    const model = await openForm()
    fireEvent.click(testButton())
    fireEvent.change(model, { target: { value: 'bad' } })
    await act(async () => { answerTest!() })
    await waitFor(() => expect(toast.info).toHaveBeenCalled())
    expect(toast.success).not.toHaveBeenCalled()
    expect(testButton().querySelector('.text-success')).toBeNull()
  })

  it('does not blame edited settings for the old draft failing', async () => {
    const model = await openForm()
    fireEvent.click(testButton())
    fireEvent.change(model, { target: { value: 'better' } })
    await act(async () => { failTest!() })
    await waitFor(() => expect(toast.info).toHaveBeenCalled())
    expect(toast.error).not.toHaveBeenCalled()
    expect(testButton().querySelector('.text-danger')).toBeNull()
  })
})
