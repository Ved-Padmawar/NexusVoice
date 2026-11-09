import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'sonner'
import { FormattingToggle } from '../../../features/settings/FormattingToggle'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)
const CONFIG = { enabled: false, provider: 'custom', profiles: { custom: { baseUrl: 'http://127.0.0.1:1/v1', model: 'good', hasApiKey: false } } }

let answerTest: (() => void) | null = null
let failTest: (() => void) | null = null

beforeEach(() => {
  vi.mocked(toast.success).mockClear()
  vi.mocked(toast.info).mockClear()
  vi.mocked(toast.error).mockClear()
  mockInvoke.mockReset().mockImplementation((cmd) => {
    if (cmd === 'get_format_config') return Promise.resolve(CONFIG)
    if (cmd === 'test_format_connection') {
      return new Promise<void>((resolve, reject) => {
        answerTest = resolve
        failTest = () => reject({ code: 'llm', message: 'model not found' })
      })
    }
    return Promise.resolve(null)
  })
})

async function openForm() {
  render(<FormattingToggle />)
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

describe('FormattingToggle save', () => {
  it('shows why Rust refused the toggle', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_format_config') return Promise.resolve(CONFIG)
      if (cmd === 'set_format_config') return Promise.reject({ code: 'io_error', message: 'settings.json is read-only' })
      return Promise.resolve(null)
    })
    render(<FormattingToggle />)
    await waitFor(() => expect(screen.getByText('good')).toBeInTheDocument())
    fireEvent.click(screen.getByRole('switch', { name: /smart formatting/i }))
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('settings.json is read-only'))
  })
})
