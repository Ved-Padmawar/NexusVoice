import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen, fireEvent, waitFor, act } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { Dashboard } from '../../pages/Dashboard'
import { useAppStore } from '../../store/useAppStore'
import { invoke } from '@tauri-apps/api/core'
import { renderWithQuery } from '../utils'
import type { Transcript, UsageStats } from '../../types'

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

const sampleTranscripts: Transcript[] = [
  { id: 1, content: 'Hello world', wordCount: 2, durationSeconds: 5, targetApp: 'VS Code', createdAt: new Date().toISOString() },
  { id: 2, content: 'Testing search', wordCount: 2, durationSeconds: 3, targetApp: null, createdAt: new Date().toISOString() },
]

const fullPage = Array.from({ length: 50 }, (_, i): Transcript => ({
  id: i + 1,
  content: `row ${i + 1}`,
  wordCount: 1,
  durationSeconds: null,
  targetApp: null,
  createdAt: `2026-01-01T00:00:${String(i + 1).padStart(2, '0')}`,
}))

type Backend = {
  transcripts?: Transcript[]
  search?: Transcript[]
  stats?: UsageStats | null
}

function mockBackend({ transcripts = [], search = [], stats = null }: Backend = {}) {
  mockInvoke.mockImplementation((cmd) => {
    if (cmd === 'get_transcripts') return Promise.resolve(transcripts)
    if (cmd === 'search_transcripts') return Promise.resolve(search)
    if (cmd === 'get_usage_stats') return Promise.resolve(stats)
    if (cmd === 'get_daily_words') return Promise.resolve([])
    return Promise.resolve(undefined)
  })
}

const renderDashboard = () =>
  renderWithQuery(<MemoryRouter><Dashboard /></MemoryRouter>)

beforeEach(() => {
  mockInvoke.mockReset()
  observed.clear()
  mockBackend()
  useAppStore.setState({ hasHotkey: true })
})

describe('Dashboard — empty state', () => {
  it('shows empty state when no transcripts', async () => {
    renderDashboard()
    expect(await screen.findByText(/your first dictation lands here/i)).toBeInTheDocument()
  })

  it('shows hotkey warning when no hotkey set', () => {
    useAppStore.setState({ hasHotkey: false })
    renderDashboard()
    expect(screen.getByText(/until a hotkey is set/i)).toBeInTheDocument()
  })
})

describe('Dashboard — transcripts', () => {
  beforeEach(() => {
    mockBackend({ transcripts: sampleTranscripts })
  })

  it('renders transcript content', async () => {
    renderDashboard()
    expect(await screen.findByText('Hello world')).toBeInTheDocument()
    expect(screen.getByText('Testing search')).toBeInTheDocument()
  })

  it('shows the day group count', async () => {
    renderDashboard()
    expect(await screen.findByText('2 dictations')).toBeInTheDocument()
  })

  it('labels a transcript with the app it was dictated into', async () => {
    renderDashboard()
    expect(await screen.findByText('VS Code')).toBeInTheDocument()
  })

  it('omits the app label when the target app is unknown', async () => {
    renderDashboard()
    await screen.findByText('Hello world')
    // The second fixture has targetApp: null, so it is labelled as the clipboard.
    expect(screen.getAllByText('VS Code')).toHaveLength(1)
    expect(screen.getAllByText('Clipboard')).toHaveLength(1)
  })
})

describe('Dashboard — search', () => {
  it('queries the backend with the debounced term', async () => {
    mockBackend({ transcripts: sampleTranscripts, search: [] })
    renderDashboard()
    await screen.findByText('Hello world')

    fireEvent.change(screen.getByLabelText('Search transcripts'), { target: { value: 'hello' } })

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('search_transcripts', expect.objectContaining({ query: 'hello' }))
    }, { timeout: 1000 })
  })

  it('shows the search empty state when nothing matches', async () => {
    mockBackend({ transcripts: sampleTranscripts, search: [] })
    renderDashboard()
    await screen.findByText('Hello world')

    fireEvent.change(screen.getByLabelText('Search transcripts'), { target: { value: 'xyz' } })

    expect(await screen.findByText(/no results found/i, {}, { timeout: 1000 })).toBeInTheDocument()
  })

  it('shows search results when the query matches', async () => {
    const hit: Transcript = { id: 9, content: 'Matched result', wordCount: 2, durationSeconds: null, targetApp: null, createdAt: new Date().toISOString() }
    mockBackend({ transcripts: [], search: [hit] })
    renderDashboard()

    fireEvent.change(screen.getByLabelText('Search transcripts'), { target: { value: 'matched' } })

    expect(await screen.findByText('Matched result', {}, { timeout: 1000 })).toBeInTheDocument()
  })
})

describe('Dashboard — stats', () => {
  it('shows stat values when stats available', async () => {
    mockBackend({ stats: { totalWords: 1234, speakingTimeSeconds: 60, totalSessions: 5, avgPaceWpm: 120 } })
    renderDashboard()
    expect(await screen.findByText('1,234')).toBeInTheDocument()
    expect(screen.getByText('1m')).toBeInTheDocument()
    expect(screen.getByText('Dictations').closest('div')).toHaveTextContent('5')
  })

  it('shows dash when stats are empty', async () => {
    mockBackend({ stats: null })
    renderDashboard()
    await waitFor(() => expect(screen.getAllByText('—').length).toBeGreaterThan(0))
  })
})

describe('Dashboard — export', () => {
  it('shows export dropdown on button click', () => {
    renderDashboard()
    fireEvent.click(screen.getByRole('button', { name: 'Export' }))
    expect(screen.getByText(/plain text/i)).toBeInTheDocument()
    expect(screen.getByText('JSON')).toBeInTheDocument()
  })

  it('pages through get_transcripts on format select', async () => {
    renderDashboard()
    fireEvent.click(screen.getByRole('button', { name: 'Export' }))
    fireEvent.click(screen.getByText(/plain text/i))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'get_transcripts',
        expect.objectContaining({ page: expect.objectContaining({ limit: 200 }) }),
      )
    })
  })
})

describe('Dashboard — filter', () => {
  it('opens filter dropdown on button click', () => {
    renderDashboard()
    fireEvent.click(screen.getByRole('button', { name: 'Filter' }))
    expect(screen.getByText(/newest first/i)).toBeInTheDocument()
  })

  it('shows range and one day toggle', () => {
    renderDashboard()
    fireEvent.click(screen.getByRole('button', { name: 'Filter' }))
    expect(screen.getByText('Range')).toBeInTheDocument()
    expect(screen.getByText('One day')).toBeInTheDocument()
  })

  it('switches to one day mode', () => {
    renderDashboard()
    fireEvent.click(screen.getByRole('button', { name: 'Filter' }))
    fireEvent.click(screen.getByText('One day'))
    // In one day mode there is a single date field
    expect(screen.getByText('On')).toBeInTheDocument()
    expect(screen.queryByText('From')).not.toBeInTheDocument()
  })

  it('refetches with the applied sort order', async () => {
    mockBackend({ transcripts: sampleTranscripts })
    renderDashboard()
    await screen.findByText('Hello world')

    fireEvent.click(screen.getByRole('button', { name: 'Filter' }))
    fireEvent.click(screen.getByText(/oldest first/i))
    fireEvent.click(screen.getByText(/^apply$/i))

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_transcripts', {
        page: expect.objectContaining({ sortAsc: true }),
      })
    })
  })
})

describe('Dashboard — infinite scroll sentinel', () => {
  it('observes the sentinel once a full page has loaded', async () => {
    mockBackend({ transcripts: fullPage })
    renderDashboard()
    await waitFor(() => expect(observed.size).toBeGreaterThan(0))
  })

  it('does not render a sentinel for a short page', async () => {
    mockBackend({ transcripts: sampleTranscripts })
    renderDashboard()
    await screen.findByText('Hello world')
    expect(observed.size).toBe(0)
  })

  it('fetches the next page from the last loaded row', async () => {
    mockBackend({ transcripts: fullPage })
    renderDashboard()
    await waitFor(() => expect(observed.size).toBeGreaterThan(0))

    mockInvoke.mockClear()
    act(() => fireIntersect?.())

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_transcripts', {
        page: expect.objectContaining({ cursorId: 50, cursorCreatedAt: '2026-01-01T00:00:50' }),
      })
    })
  })

  it('renders a row once when the next page repeats it', async () => {
    let call = 0
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_transcripts') return Promise.resolve(call++ === 0 ? fullPage : [fullPage[49]])
      if (cmd === 'get_usage_stats') return Promise.resolve(null)
      if (cmd === 'get_daily_words') return Promise.resolve([])
      return Promise.resolve(undefined)
    })
    renderDashboard()
    await waitFor(() => expect(observed.size).toBeGreaterThan(0))
    act(() => fireIntersect?.())
    await waitFor(() => expect(call).toBe(2))
    expect(await screen.findAllByText('row 50')).toHaveLength(1)
  })
})

describe('Dashboard — database failure', () => {
  it('shows one error when stats and activity both fail, and retries both', async () => {
    mockInvoke.mockRejectedValue({ code: 'database', message: 'database unavailable' })
    renderDashboard()
    await waitFor(() => expect(screen.getAllByRole('alert')).toHaveLength(1))

    const statsCalls = () => mockInvoke.mock.calls.filter(([cmd]) => cmd === 'get_usage_stats').length
    const before = statsCalls()
    mockBackend({ transcripts: sampleTranscripts })
    fireEvent.click(screen.getByRole('button', { name: /try again/i }))
    expect(await screen.findByText('Hello world')).toBeInTheDocument()
    expect(statsCalls()).toBeGreaterThan(before)
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })
})
