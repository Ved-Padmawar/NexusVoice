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

const openNewRow = async () => {
  fireEvent.click(screen.getByRole('button', { name: /add word/i }))
  await screen.findByPlaceholderText('Heard word')
}
const termField = () => screen.getByPlaceholderText('Heard word')
const replacementField = () => screen.getByPlaceholderText('Replacement')
const saveButton = () => screen.getByRole('button', { name: /^add$/i })

beforeEach(() => {
  mockInvoke.mockReset()
  mockBackend()
})

describe('Dictionary — empty state', () => {
  it('shows empty state when no entries', async () => {
    render()
    expect(await screen.findByText(/no corrections yet/i)).toBeInTheDocument()
  })
})

describe('Dictionary — add entry', () => {
  it('keeps failed additions available for retry without an unhandled rejection', async () => {
    mockInvoke.mockImplementation(cmd => cmd === 'update_dictionary' ? Promise.reject(new Error('Write failed')) : Promise.resolve([]))
    render()
    await openNewRow()
    fireEvent.change(termField(), { target: { value: 'teh' } })
    fireEvent.change(replacementField(), { target: { value: 'the' } })
    fireEvent.click(saveButton())
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Write failed'))
    expect(screen.getByDisplayValue('teh')).toBeEnabled()
    expect(screen.getByDisplayValue('the')).toBeEnabled()
  })

  it('Add button is disabled when inputs are empty', async () => {
    render()
    await openNewRow()
    expect(saveButton()).toBeDisabled()
  })

  it('sends the term and replacement to the backend', async () => {
    render()
    await openNewRow()
    fireEvent.change(termField(), { target: { value: 'teh' } })
    fireEvent.change(replacementField(), { target: { value: 'the' } })
    fireEvent.click(saveButton())
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('update_dictionary', { term: 'teh', replacement: 'the', previousTerm: null })
    })
  })

  it('closes the new row after a successful add', async () => {
    render()
    await openNewRow()
    fireEvent.change(termField(), { target: { value: 'teh' } })
    fireEvent.change(replacementField(), { target: { value: 'the' } })
    fireEvent.click(saveButton())
    // The new row closes once the entry is saved.
    await waitFor(() => expect(screen.queryByPlaceholderText('Heard word')).not.toBeInTheDocument())
  })

  it('submits on Enter key in replacement input', async () => {
    render()
    await openNewRow()
    fireEvent.change(termField(), { target: { value: 'gonna' } })
    const replacementInput = replacementField()
    fireEvent.change(replacementInput, { target: { value: 'going to' } })
    fireEvent.keyDown(replacementInput, { key: 'Enter' })
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('update_dictionary', { term: 'gonna', replacement: 'going to', previousTerm: null })
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
    fireEvent.click(screen.getByRole('button', { name: /edit teh/i }))
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
    const deleteBtn = screen.getByRole('button', { name: /delete teh/i })
    fireEvent.click(deleteBtn)
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('delete_dictionary_entry', { id: 1 })
    })
  })

  it('sends the previous term when an entry is renamed', async () => {
    render()
    await screen.findByText('teh')
    const editBtn = screen.getByRole('button', { name: /edit teh/i })
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
    const editBtn = screen.getByRole('button', { name: /edit teh/i })
    if (editBtn) fireEvent.click(editBtn)
    expect(screen.getAllByRole('textbox').length).toBeGreaterThan(0)
  })

  it('cancels edit mode on X click', async () => {
    render()
    await screen.findByText('teh')
    const editBtn = screen.getByRole('button', { name: /edit teh/i })
    if (editBtn) fireEvent.click(editBtn)
    // After entering edit mode, find the X (cancel) button
    const cancelBtn = screen.getByRole('button', { name: /^cancel$/i })
    if (cancelBtn) fireEvent.click(cancelBtn)
    expect(screen.getByText('teh')).toBeInTheDocument()
  })
})
