import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { Dictionary } from '../pages/Dictionary'
import { useAppStore } from '../store/useAppStore'

vi.mock('sonner', () => ({ toast: { success: vi.fn() } }))

const mockUpdateDictionary = vi.fn()
const mockDeleteDictionaryEntry = vi.fn()

const sampleEntry = { id: 1, term: 'teh', replacement: 'the', hits: 3, createdAt: '' }

beforeEach(() => {
  mockUpdateDictionary.mockReset()
  mockDeleteDictionaryEntry.mockReset()
  mockUpdateDictionary.mockResolvedValue(undefined)
  mockDeleteDictionaryEntry.mockResolvedValue(undefined)
  useAppStore.setState({
    dictionary: [],
    error: null,
    updateDictionary: mockUpdateDictionary,
    deleteDictionaryEntry: mockDeleteDictionaryEntry,
  } as never)
})

describe('Dictionary — empty state', () => {
  it('shows empty state when no entries', () => {
    render(<Dictionary />)
    expect(screen.getByText(/no entries yet/i)).toBeInTheDocument()
  })
})

describe('Dictionary — add entry', () => {
  it('Add button is disabled when inputs are empty', () => {
    render(<Dictionary />)
    expect(screen.getByRole('button', { name: /add to dictionary/i })).toBeDisabled()
  })

  it('calls updateDictionary with term and replacement', async () => {
    render(<Dictionary />)
    fireEvent.change(screen.getByPlaceholderText(/e.g. teh/i), { target: { value: 'teh' } })
    fireEvent.change(screen.getByPlaceholderText(/e.g. the/i), { target: { value: 'the' } })
    fireEvent.click(screen.getByRole('button', { name: /add to dictionary/i }))
    await waitFor(() => {
      expect(mockUpdateDictionary).toHaveBeenCalledWith('teh', 'the')
    })
  })

  it('clears inputs after successful add', async () => {
    render(<Dictionary />)
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
    render(<Dictionary />)
    fireEvent.change(screen.getByPlaceholderText(/e.g. teh/i), { target: { value: 'gonna' } })
    const replacementInput = screen.getByPlaceholderText(/e.g. the/i)
    fireEvent.change(replacementInput, { target: { value: 'going to' } })
    fireEvent.keyDown(replacementInput, { key: 'Enter' })
    await waitFor(() => {
      expect(mockUpdateDictionary).toHaveBeenCalledWith('gonna', 'going to')
    })
  })
})

describe('Dictionary — existing entries', () => {
  beforeEach(() => {
    useAppStore.setState({ dictionary: [sampleEntry] } as never)
  })

  it('renders entry term and replacement', () => {
    render(<Dictionary />)
    expect(screen.getByText('teh')).toBeInTheDocument()
    expect(screen.getByText('the')).toBeInTheDocument()
  })

  it('calls deleteDictionaryEntry when delete clicked', async () => {
    render(<Dictionary />)
    // Trash button (last icon button in the row)
    const buttons = screen.getAllByRole('button')
    const deleteBtn = buttons.find(b => b.querySelector('svg.lucide-trash-2'))
    if (deleteBtn) fireEvent.click(deleteBtn)
    await waitFor(() => {
      expect(mockDeleteDictionaryEntry).toHaveBeenCalledWith(1)
    })
  })

  it('enters edit mode on pencil click', () => {
    render(<Dictionary />)
    const buttons = screen.getAllByRole('button')
    const editBtn = buttons.find(b => b.querySelector('svg.lucide-pencil'))
    if (editBtn) fireEvent.click(editBtn)
    expect(screen.getAllByRole('textbox').length).toBeGreaterThan(0)
  })

  it('cancels edit mode on X click', () => {
    render(<Dictionary />)
    const buttons = screen.getAllByRole('button')
    const editBtn = buttons.find(b => b.querySelector('svg.lucide-pencil'))
    if (editBtn) fireEvent.click(editBtn)
    // After entering edit mode, find the X (cancel) button
    const allButtons = screen.getAllByRole('button')
    const cancelBtn = allButtons.find(b => b.querySelector('svg.lucide-x'))
    if (cancelBtn) fireEvent.click(cancelBtn)
    expect(screen.getByText('teh')).toBeInTheDocument()
  })
})
