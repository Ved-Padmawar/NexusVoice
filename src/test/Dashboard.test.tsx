import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { Dashboard } from '../pages/Dashboard'
import { useAppStore } from '../store/useAppStore'
import { invoke } from '@tauri-apps/api/core'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { success: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

const sampleTranscripts = [
  { id: 1, content: 'Hello world', wordCount: 2, durationSeconds: 5, createdAt: new Date().toISOString() },
  { id: 2, content: 'Testing search', wordCount: 2, durationSeconds: 3, createdAt: new Date().toISOString() },
]

function renderDashboard() {
  return render(
    <MemoryRouter>
      <Dashboard />
    </MemoryRouter>
  )
}

beforeEach(() => {
  mockInvoke.mockReset()
  mockInvoke.mockResolvedValue(undefined)
  useAppStore.setState({
    transcripts: [],
    transcriptHasMore: false,
    transcriptOffset: 0,
    searchResults: [],
    isSearching: false,
    searchQuery: '',
    filterFrom: null,
    filterTo: null,
    filterSortAsc: false,
    stats: null,
    hasHotkey: true,
  })
})

describe('Dashboard — empty state', () => {
  it('shows empty state when no transcripts', () => {
    renderDashboard()
    expect(screen.getByText(/nothing here yet/i)).toBeInTheDocument()
  })

  it('shows hotkey warning when no hotkey set', () => {
    useAppStore.setState({ hasHotkey: false })
    renderDashboard()
    expect(screen.getByText(/no hotkey set/i)).toBeInTheDocument()
  })
})

describe('Dashboard — transcripts', () => {
  beforeEach(() => {
    useAppStore.setState({ transcripts: sampleTranscripts })
  })

  it('renders transcript content', () => {
    renderDashboard()
    expect(screen.getByText('Hello world')).toBeInTheDocument()
    expect(screen.getByText('Testing search')).toBeInTheDocument()
  })

  it('shows transcript count badge', () => {
    renderDashboard()
    expect(screen.getByText('2')).toBeInTheDocument()
  })
})

describe('Dashboard — search', () => {
  it('calls searchTranscripts on input', async () => {
    const mockSearch = vi.fn()
    useAppStore.setState({ searchTranscripts: mockSearch } as never)
    renderDashboard()
    const input = screen.getByPlaceholderText(/search transcripts/i)
    fireEvent.change(input, { target: { value: 'hello' } })
    await waitFor(() => {
      expect(mockSearch).toHaveBeenCalledWith('hello')
    }, { timeout: 500 })
  })

  it('shows search empty state when no results', () => {
    useAppStore.setState({ searchResults: [], isSearching: false })
    renderDashboard()
    const input = screen.getByPlaceholderText(/search transcripts/i)
    fireEvent.change(input, { target: { value: 'xyz' } })
    // searchResults is empty and query is set — empty state should show
    expect(screen.getByText(/nothing here yet|no results/i)).toBeInTheDocument()
  })

  it('shows search results when query matches', () => {
    useAppStore.setState({
      searchResults: [{ id: 1, content: 'Hello world', wordCount: 2, durationSeconds: null, createdAt: '' }],
      isSearching: false,
    })
    renderDashboard()
    const input = screen.getByPlaceholderText(/search transcripts/i)
    fireEvent.change(input, { target: { value: 'hello' } })
    expect(screen.getByText('Hello world')).toBeInTheDocument()
  })
})

describe('Dashboard — stats', () => {
  it('shows stat values when stats available', () => {
    useAppStore.setState({
      stats: { totalWords: 1234, speakingTimeSeconds: 60, totalSessions: 5, avgPaceWpm: 120 },
    })
    renderDashboard()
    expect(screen.getByText('1,234')).toBeInTheDocument()
    expect(screen.getByText('1m')).toBeInTheDocument()
    expect(screen.getByText('5')).toBeInTheDocument()
  })

  it('shows dash when stats not loaded', () => {
    useAppStore.setState({ stats: null })
    renderDashboard()
    const dashes = screen.getAllByText('—')
    expect(dashes.length).toBeGreaterThan(0)
  })
})

describe('Dashboard — export', () => {
  it('shows export dropdown on button click', () => {
    renderDashboard()
    fireEvent.click(screen.getByTitle(/export transcripts/i))
    expect(screen.getByText(/plain text/i)).toBeInTheDocument()
    expect(screen.getByText(/json/i)).toBeInTheDocument()
  })

  it('calls export_transcripts on format select', async () => {
    mockInvoke.mockResolvedValue([])
    renderDashboard()
    fireEvent.click(screen.getByTitle(/export transcripts/i))
    fireEvent.click(screen.getByText(/plain text/i))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('export_transcripts')
    })
  })
})

describe('Dashboard — filter', () => {
  it('opens filter dropdown on button click', () => {
    renderDashboard()
    fireEvent.click(screen.getByText(/^filter/i))
    expect(screen.getByText(/newest first/i)).toBeInTheDocument()
  })

  it('shows range and specific day toggle', () => {
    renderDashboard()
    fireEvent.click(screen.getByText(/^filter/i))
    expect(screen.getByText('Range')).toBeInTheDocument()
    expect(screen.getByText(/specific day/i)).toBeInTheDocument()
  })

  it('switches to specific day mode', () => {
    renderDashboard()
    fireEvent.click(screen.getByText(/^filter/i))
    fireEvent.click(screen.getByText(/specific day/i))
    // In specific day mode there is only one date input
    const dateInputs = screen.getAllByDisplayValue('')
    expect(dateInputs.length).toBeGreaterThan(0)
  })
})
