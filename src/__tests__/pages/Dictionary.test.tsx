import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen, fireEvent, waitFor } from '@testing-library/react'
import { Dictionary } from '../../pages/Dictionary'
import { invoke } from '@tauri-apps/api/core'
import { renderWithQuery } from '../utils'
import type { DictionaryEntry } from '../../types'
import { toast } from 'sonner'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

const sampleEntry: DictionaryEntry = { id: 1, term: 'teh', replacement: 'the', hits: 3, createdAt: '' }

function mockBackend(entries: DictionaryEntry[] = []) {
  mockInvoke.mockImplementation((cmd) => {
    if (cmd === 'get_dictionary') return Promise.resolve(entries)
    if (cmd === 'update_dictionary') return Promise.resolve(sampleEntry)
    return Promise.resolve(undefined)
  })
}

const render = () => renderWithQuery(<Dictionary />)

beforeEach(() => {
  mockInvoke.mockReset()
  mockBackend()
})

describe('Dictionary — empty state', () => {
  it('shows empty state when no entries', async () => {
    render()
    expect(await screen.findByText(/no entries yet/i)).toBeInTheDocument()
  })
})

describe('Dictionary — add entry', () => {
  it('keeps failed additions available for retry without an unhandled rejection', async () => {
    mockInvoke.mockImplementation(cmd => cmd === 'update_dictionary' ? Promise.reject(new Error('Write failed')) : Promise.resolve([]))
    render()
    fireEvent.change(screen.getByPlaceholderText(/e.g. teh/i), { target: { value: 'teh' } })
    fireEvent.change(screen.getByPlaceholderText(/e.g. the/i), { target: { value: 'the' } })
    fireEvent.click(screen.getByRole('button', { name: /add to dictionary/i }))
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Write failed'))
    expect(screen.getByDisplayValue('teh')).toBeEnabled()
    expect(screen.getByDisplayValue('the')).toBeEnabled()
  })

  it('Add button is disabled when inputs are empty', () => {
    render()
    expect(screen.getByRole('button', { name: /add to dictionary/i })).toBeDisabled()
  })

  it('sends the term and replacement to the backend', async () => {
    render()
    fireEvent.change(screen.getByPlaceholderText(/e.g. teh/i), { target: { value: 'teh' } })
    fireEvent.change(screen.getByPlaceholderText(/e.g. the/i), { target: { value: 'the' } })
    fireEvent.click(screen.getByRole('button', { name: /add to dictionary/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('update_dictionary', { term: 'teh', replacement: 'the' })
    })
  })

  it('clears inputs after successful add', async () => {
    render()
    const termInput = screen.getByPlaceholderText(/e.g. teh/i)
    const replacementInput = screen.getByPlaceholderText(/e.g. the/i)
    fireEvent.change(termInput, { target: { value: 'teh' } })
    fireEvent.change(replacementInput, { target: { value: 'the' } })
    fireEvent.click(screen.getByRole('button', { name: /add to dictionary/i }))
    await waitFor(() => {
      expect((termInput as HTMLInputElement).value).toBe('')
      expect((replacementInput as HTMLInputElement).value).toBe('')
    })
  })

  it('submits on Enter key in replacement input', async () => {
    render()
    fireEvent.change(screen.getByPlaceholderText(/e.g. teh/i), { target: { value: 'gonna' } })
    const replacementInput = screen.getByPlaceholderText(/e.g. the/i)
    fireEvent.change(replacementInput, { target: { value: 'going to' } })
    fireEvent.keyDown(replacementInput, { key: 'Enter' })
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('update_dictionary', { term: 'gonna', replacement: 'going to' })
    })
  })
})

describe('Dictionary — existing entries', () => {
  beforeEach(() => {
    mockBackend([sampleEntry])
  })

  it('preserves a failed edit for retry without an unhandled rejection', async () => {
    mockInvoke.mockImplementation(cmd => cmd === 'update_dictionary' ? Promise.reject(new Error('Edit failed')) : Promise.resolve([sampleEntry]))
    render()
    await screen.findByText('teh')
    fireEvent.click(screen.getAllByRole('button').find(b => b.querySelector('svg.lucide-pencil'))!)
    fireEvent.change(screen.getByDisplayValue('teh'), { target: { value: 'hte' } })
    fireEvent.keyDown(screen.getByDisplayValue('the'), { key: 'Enter' })
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Edit failed'))
    expect(screen.getByDisplayValue('hte')).toBeEnabled()
    expect(screen.getByDisplayValue('the')).toBeEnabled()
  })

  it('renders entry term and replacement', async () => {
    render()
    expect(await screen.findByText('teh')).toBeInTheDocument()
    expect(screen.getByText('the')).toBeInTheDocument()
  })

  it('deletes the entry by id when delete is clicked', async () => {
    render()
    await screen.findByText('teh')
    // Trash button (last icon button in the row)
    const deleteBtn = screen.getAllByRole('button').find(b => b.querySelector('svg.lucide-trash-2'))
    if (deleteBtn) fireEvent.click(deleteBtn)
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('delete_dictionary_entry', { id: 1 })
    })
  })

  it('sends the previous term when an entry is renamed', async () => {
    render()
    await screen.findByText('teh')
    const editBtn = screen.getAllByRole('button').find(b => b.querySelector('svg.lucide-pencil'))
    fireEvent.click(editBtn!)

    const termInput = await screen.findByDisplayValue('teh')
    fireEvent.change(termInput, { target: { value: 'hte' } })
    fireEvent.keyDown(screen.getByDisplayValue('the'), { key: 'Enter' })

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('update_dictionary', {
        term: 'hte',
        replacement: 'the',
        previousTerm: 'teh',
      })
    })
  })

  it('enters edit mode on pencil click', async () => {
    render()
    await screen.findByText('teh')
    const editBtn = screen.getAllByRole('button').find(b => b.querySelector('svg.lucide-pencil'))
    if (editBtn) fireEvent.click(editBtn)
    expect(screen.getAllByRole('textbox').length).toBeGreaterThan(0)
  })

  it('cancels edit mode on X click', async () => {
    render()
    await screen.findByText('teh')
    const editBtn = screen.getAllByRole('button').find(b => b.querySelector('svg.lucide-pencil'))
    if (editBtn) fireEvent.click(editBtn)
    // After entering edit mode, find the X (cancel) button
    const cancelBtn = screen.getAllByRole('button').find(b => b.querySelector('svg.lucide-x'))
    if (cancelBtn) fireEvent.click(cancelBtn)
    expect(screen.getByText('teh')).toBeInTheDocument()
  })
})
