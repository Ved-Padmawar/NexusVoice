import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { Dashboard } from '../../pages/Dashboard'
import { useAppStore } from '../../store/useAppStore'
import { invoke } from '@tauri-apps/api/core'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

// downloadBlob clicks an anchor; jsdom can't navigate and logs about it.
vi.mock('../../lib/utils', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../lib/utils')>()),
  downloadBlob: vi.fn(),
}))

const mockInvoke = vi.mocked(invoke)

// jsdom has no IntersectionObserver; this one records observed nodes.
const observed = new Set<Element>()
let fireIntersect: (() => void) | null = null

class MockIntersectionObserver {
  constructor(cb: IntersectionObserverCallback) {
    fireIntersect = () => {
      for (const el of observed) {
        cb([{ isIntersecting: true, target: el } as IntersectionObserverEntry], this as never)
      }
    }
  }
  observe(el: Element) { observed.add(el) }
  unobserve(el: Element) { observed.delete(el) }
  disconnect() { observed.clear() }
}
vi.stubGlobal('IntersectionObserver', MockIntersectionObserver)

const sampleTranscripts = [
  { id: 1, content: 'Hello world', wordCount: 2, durationSeconds: 5, targetApp: 'VS Code', createdAt: new Date().toISOString() },
  { id: 2, content: 'Testing search', wordCount: 2, durationSeconds: 3, targetApp: null, createdAt: new Date().toISOString() },
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
    transcriptLoadingMore: false,
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

  it('labels a transcript with the app it was dictated into', () => {
    renderDashboard()
    expect(screen.getByText(/Pasted in VS Code/)).toBeInTheDocument()
  })

  it('omits the app label when the target app is unknown', () => {
    renderDashboard()
    // The second fixture has targetApp: null — only one label should render.
    expect(screen.getAllByText(/Pasted in/)).toHaveLength(1)
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
      searchResults: [{ id: 1, content: 'Hello world', wordCount: 2, durationSeconds: null, targetApp: null, createdAt: '' }],
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

describe('Dashboard — infinite scroll sentinel', () => {
  it('observes the sentinel that appears after the loading skeleton clears', async () => {
    // The skeleton means no sentinel on first mount; an effect keyed on
    // hasMore/searchMode would read a null ref and never re-run.
    useAppStore.setState({
      transcripts: [],
      transcriptHasMore: true,
      transcriptsStatus: 'loading',
    })
    renderDashboard()
    expect(observed.size).toBe(0)

    act(() => {
      useAppStore.setState({
        transcripts: sampleTranscripts,
        transcriptsStatus: 'success',
      })
    })

    await waitFor(() => expect(observed.size).toBeGreaterThan(0))
  })

  it('loads more when the sentinel scrolls into view', async () => {
    useAppStore.setState({
      transcripts: sampleTranscripts,
      transcriptHasMore: true,
      transcriptsStatus: 'success',
    })
    const spy = vi.spyOn(useAppStore.getState(), 'loadMoreTranscripts').mockResolvedValue()
    renderDashboard()
    await waitFor(() => expect(observed.size).toBeGreaterThan(0))

    act(() => fireIntersect?.())
    expect(spy).toHaveBeenCalled()
    spy.mockRestore()
  })
})
