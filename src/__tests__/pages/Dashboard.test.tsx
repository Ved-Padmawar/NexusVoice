import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen, fireEvent, waitFor, act } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { Dashboard } from '../../pages/Dashboard'
import { useAppStore } from '../../store/useAppStore'
import { invoke } from '@tauri-apps/api/core'
import { renderWithQuery } from '../utils'
import { toUtcBounds } from '../../lib/dates'
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

const pad = (n: number) => String(n).padStart(2, '0')
const now = new Date()
const todayIso = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`
/** A fixed day in the currently displayed month, for range clicks. */
const dayInThisMonth = (d: number) => `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(d)}`

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
    return Promise.resolve(undefined)
  })
}

const renderDashboard = () =>
  renderWithQuery(<MemoryRouter><Dashboard /></MemoryRouter>)

beforeEach(() => {
  mockInvoke.mockReset()
  observed.clear()
  mockBackend()
  useAppStore.setState({ hasHotkey: true, modelReady: true, activeModelName: null, downloads: {} })
})

describe('Dashboard — empty state', () => {
  it('shows empty state when no transcripts', async () => {
    renderDashboard()
    expect(await screen.findByText(/nothing here yet/i)).toBeInTheDocument()
  })

  // The armed/not-armed banner moved to the status lamp; the masthead still
  // owes the user the next action.
  it('points at the hotkey setting when no hotkey is set', () => {
    useAppStore.setState({ hasHotkey: false, modelReady: true })
    renderDashboard()
    expect(screen.getByText(/set a hotkey in settings/i)).toBeInTheDocument()
  })

  // The catalogue names a model even with nothing on disk; leading with that
  // name read as "you have this one" next to a prompt to go get one.
  it('demotes the recommended model instead of implying it is loaded', () => {
    useAppStore.setState({
      hasHotkey: true,
      modelReady: false,
      activeModelName: 'Whisper Large v3 Turbo',
    })
    renderDashboard()
    expect(screen.getByText('No model')).toBeInTheDocument()
    expect(screen.getByText(/whisper large v3 turbo recommended/i)).toBeInTheDocument()
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

  it('shows transcript count badge', async () => {
    renderDashboard()
    expect(await screen.findByText('2')).toBeInTheDocument()
  })

  it('labels a transcript with the app it was dictated into', async () => {
    renderDashboard()
    expect(await screen.findByText(/Pasted in VS Code/)).toBeInTheDocument()
  })

  it('omits the app label when the target app is unknown', async () => {
    renderDashboard()
    await screen.findByText('Hello world')
    // The second fixture has targetApp: null — only one label should render.
    expect(screen.getAllByText(/Pasted in/)).toHaveLength(1)
  })
})

describe('Dashboard — search', () => {
  it('queries the backend with the debounced term', async () => {
    mockBackend({ transcripts: sampleTranscripts, search: [] })
    renderDashboard()
    await screen.findByText('Hello world')

    fireEvent.change(screen.getByPlaceholderText(/search transcripts/i), { target: { value: 'hello' } })

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('search_transcripts', expect.objectContaining({ query: 'hello' }))
    }, { timeout: 1000 })
  })

  it('shows the search empty state when nothing matches', async () => {
    mockBackend({ transcripts: sampleTranscripts, search: [] })
    renderDashboard()
    await screen.findByText('Hello world')

    fireEvent.change(screen.getByPlaceholderText(/search transcripts/i), { target: { value: 'xyz' } })

    expect(await screen.findByText(/no results found/i, {}, { timeout: 1000 })).toBeInTheDocument()
  })

  it('shows search results when the query matches', async () => {
    const hit: Transcript = { id: 9, content: 'Matched result', wordCount: 2, durationSeconds: null, targetApp: null, createdAt: new Date().toISOString() }
    mockBackend({ transcripts: [], search: [hit] })
    renderDashboard()

    fireEvent.change(screen.getByPlaceholderText(/search transcripts/i), { target: { value: 'matched' } })

    expect(await screen.findByText('Matched result', {}, { timeout: 1000 })).toBeInTheDocument()
  })
})

describe('Dashboard — stats', () => {
  it('shows stat values when stats available', async () => {
    mockBackend({ stats: { totalWords: 1234, speakingTimeSeconds: 60, totalSessions: 5, avgPaceWpm: 120 } })
    renderDashboard()
    expect(await screen.findByText('1,234')).toBeInTheDocument()
    expect(screen.getByText('1m')).toBeInTheDocument()
    expect(screen.getByText('5')).toBeInTheDocument()
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
    fireEvent.click(screen.getByTitle(/export transcripts/i))
    expect(screen.getByRole('button', { name: /plain text/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /json/i })).toBeInTheDocument()
  })

  it('calls export_transcripts on format select', async () => {
    renderDashboard()
    fireEvent.click(screen.getByTitle(/export transcripts/i))
    fireEvent.click(screen.getByRole('button', { name: /plain text/i }))
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

  it('offers the recent-window presets', () => {
    renderDashboard()
    fireEvent.click(screen.getByText(/^filter/i))
    expect(screen.getByRole('button', { name: 'Today' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '7 days' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '30 days' })).toBeInTheDocument()
  })

  it('shows a month grid with a paging header', () => {
    renderDashboard()
    fireEvent.click(screen.getByText(/^filter/i))
    expect(screen.getByRole('button', { name: /previous month/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /next month/i })).toBeInTheDocument()
    // Days are labelled with their own ISO date.
    expect(screen.getByRole('button', { name: todayIso })).toBeInTheDocument()
  })

  it('applies a preset window immediately, without a confirm step', async () => {
    mockBackend({ transcripts: sampleTranscripts })
    renderDashboard()
    await screen.findByText('Hello world')

    fireEvent.click(screen.getByText(/^filter/i))
    fireEvent.click(screen.getByRole('button', { name: 'Today' }))

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_transcripts', {
        page: expect.objectContaining(toUtcBounds(todayIso, todayIso)),
      })
    })
  })

  it('builds a range from two day clicks', async () => {
    mockBackend({ transcripts: sampleTranscripts })
    renderDashboard()
    await screen.findByText('Hello world')

    fireEvent.click(screen.getByText(/^filter/i))
    const first = screen.getByRole('button', { name: dayInThisMonth(3) })
    const second = screen.getByRole('button', { name: dayInThisMonth(9) })
    fireEvent.click(first)
    fireEvent.click(second)

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_transcripts', {
        page: expect.objectContaining(toUtcBounds(dayInThisMonth(3), dayInThisMonth(9))),
      })
    })
  })

  it('normalises a backwards range instead of rejecting it', async () => {
    mockBackend({ transcripts: sampleTranscripts })
    renderDashboard()
    await screen.findByText('Hello world')

    fireEvent.click(screen.getByText(/^filter/i))
    fireEvent.click(screen.getByRole('button', { name: dayInThisMonth(9) }))
    fireEvent.click(screen.getByRole('button', { name: dayInThisMonth(3) }))

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_transcripts', {
        page: expect.objectContaining(toUtcBounds(dayInThisMonth(3), dayInThisMonth(9))),
      })
    })
  })

  it('refetches with the applied sort order', async () => {
    mockBackend({ transcripts: sampleTranscripts })
    renderDashboard()
    await screen.findByText('Hello world')

    fireEvent.click(screen.getByText(/^filter/i))
    fireEvent.click(screen.getByText(/oldest first/i))

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_transcripts', {
        page: expect.objectContaining({ sortAsc: true }),
      })
    })
  })

  it('clears back to no filters', async () => {
    mockBackend({ transcripts: sampleTranscripts })
    renderDashboard()
    await screen.findByText('Hello world')

    fireEvent.click(screen.getByText(/^filter/i))
    fireEvent.click(screen.getByRole('button', { name: 'Today' }))
    await waitFor(() => expect(screen.getByRole('button', { name: /clear/i })).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /clear/i }))

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_transcripts', {
        page: expect.objectContaining({ from: null, to: null }),
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
      return Promise.resolve(undefined)
    })
    renderDashboard()
    await waitFor(() => expect(observed.size).toBeGreaterThan(0))
    act(() => fireIntersect?.())
    await waitFor(() => expect(call).toBe(2))
    expect(await screen.findAllByText('row 50')).toHaveLength(1)
  })
})
