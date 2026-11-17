import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen, fireEvent, waitFor, act } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { Dashboard } from '../../../features/history/Dashboard'
import { invoke } from '@tauri-apps/api/core'
import { mirror, renderWithQuery, settingsAt, type Send } from '../../utils'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import type { Transcript, UsageStats } from '../../../shared/types'

vi.mock('@tauri-apps/api/webview', () => ({ getCurrentWebview: () => ({ onDragDropEvent: () => Promise.resolve(() => {}) }) }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() } }))

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
  { id: 1, content: 'Hello world', wordCount: 2, durationSeconds: 5, targetApp: 'VS Code', pinned: false, audioPath: null, sourceName: null, model: null, pending: false, revision: 0, createdAt: new Date().toISOString() },
  { id: 2, content: 'Testing search', wordCount: 2, durationSeconds: 3, targetApp: null, pinned: false, audioPath: null, sourceName: null, model: null, pending: false, revision: 0, createdAt: new Date().toISOString() },
]

const fullPage = Array.from({ length: 50 }, (_, i): Transcript => ({
  id: i + 1,
  content: `row ${i + 1}`,
  wordCount: 1,
  durationSeconds: null,
  targetApp: null, pinned: false, audioPath: null, sourceName: null, model: null, pending: false, revision: 0,
  createdAt: `2026-01-01T00:00:${String(i + 1).padStart(2, '0')}`,
}))

type Backend = {
  transcripts?: Transcript[]
  search?: Transcript[]
  stats?: UsageStats | null
  pushToTalk?: string | null
}

function mockBackend({ transcripts = [], search = [], stats = null, pushToTalk = 'Ctrl+Space' }: Backend = {}) {
  mockInvoke.mockImplementation((cmd) => {
    if (cmd === 'get_transcripts') return Promise.resolve(transcripts)
    if (cmd === 'search_transcripts') return Promise.resolve(search)
    if (cmd === 'get_usage_stats') return Promise.resolve(stats)
    if (cmd === 'get_daily_words') return Promise.resolve([])
    if (cmd === 'get_history_apps') return Promise.resolve(['Slack', 'Notes'])
    if (cmd === 'get_jobs') return Promise.resolve([])
    if (cmd === 'get_downloaded_models') return Promise.resolve([])
    if (cmd === 'get_hotkeys') return Promise.resolve({ pushToTalk, dictation: null, dictationCommit: null })
    return Promise.resolve(undefined)
  })
}

const renderDashboard = () =>
  renderWithQuery(<MemoryRouter><Dashboard /></MemoryRouter>)

let send: Send
let writes = 0
/** A write to History committed in Rust. */
const historyChanged = () => send({ slice: 'history', rev: ++writes, value: writes })

beforeEach(async () => {
  mockInvoke.mockReset()
  observed.clear()
  mockBackend()
  vi.mocked(listen).mockResolvedValue(() => {})
  writes = 0
  send = await mirror()
})

it('imports with the active model, in the language picked over the saved one', async () => {
  vi.mocked(open).mockResolvedValueOnce('sample.wav')
  const backend = mockInvoke.getMockImplementation()!
  mockInvoke.mockImplementation((cmd, args) => cmd === 'inspect_audio_file'
    ? Promise.resolve({ name: 'sample.wav', sizeBytes: 1024, durationSeconds: 2 })
    : backend(cmd, args))
  await send(settingsAt(1, { language: { supported: true, options: [
    { code: 'en', name: 'English', isSelected: true },
    { code: 'fr', name: 'French', isSelected: false },
  ] } }))
  renderDashboard()
  fireEvent.click(screen.getByRole('button', { name: 'Import audio' }))
  const start = await screen.findByRole('button', { name: 'Transcribe' })

  fireEvent.click(screen.getByRole('button', { name: 'Language' }))
  fireEvent.click(screen.getByRole('option', { name: 'French' }))
  fireEvent.click(start)
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('import_audio_file', { path: 'sample.wav', language: 'fr' }))
})

describe('Dashboard — empty state', () => {
  it('shows empty state when no transcripts', async () => {
    renderDashboard()
    expect(await screen.findByText(/your first dictation lands here/i)).toBeInTheDocument()
  })

  it('shows hotkey warning when no hotkey set', async () => {
    mockBackend({ pushToTalk: null })
    renderDashboard()
    expect(await screen.findByText(/until a hotkey is set/i)).toBeInTheDocument()
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

  it('omits the app label when the target app is unknown', async () => {
    renderDashboard()
    await screen.findByText('Hello world')
    // The second fixture has targetApp: null, pinned: false, audioPath: null, sourceName: null, model: null, so it is labelled as the clipboard.
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
    const hit: Transcript = { id: 9, content: 'Matched result', wordCount: 2, durationSeconds: null, targetApp: null, pinned: false, audioPath: null, sourceName: null, model: null, pending: false, revision: 0, createdAt: new Date().toISOString() }
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
  it('exports what the search box says, even before the list catches up', async () => {
    mockBackend({ transcripts: sampleTranscripts, search: [sampleTranscripts[1]] })
    renderDashboard()
    await screen.findByText('Hello world')
    fireEvent.change(await screen.findByRole('searchbox', { name: /search transcripts/i }), { target: { value: 'search' } })
    fireEvent.click(screen.getByRole('button', { name: 'Export' }))
    fireEvent.click(screen.getByText(/plain text/i))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('export_transcripts', {
        request: expect.objectContaining({ query: 'search', format: 'txt' }),
      })
    })
  })
})

describe('Dashboard — filter', () => {
  it('switches to one day mode', () => {
    renderDashboard()
    fireEvent.click(screen.getByRole('button', { name: 'Filter' }))
    fireEvent.click(screen.getByText('One day'))
    // In one day mode there is a single date field
    expect(screen.getByText('On')).toBeInTheDocument()
    expect(screen.queryByText('From')).not.toBeInTheDocument()
  })

  const applyTodayAsOneDay = () => {
    fireEvent.click(screen.getByRole('button', { name: 'Filter' }))
    fireEvent.click(screen.getByText('One day'))
    fireEvent.click(screen.getByRole('button', { name: /pick a date/i }))
    fireEvent.click(screen.getByRole('button', { name: 'Today' }))
    fireEvent.click(screen.getByText(/^apply$/i))
  }

  it('reopens a one-day filter showing its day', () => {
    renderDashboard()
    applyTodayAsOneDay()

    fireEvent.click(screen.getByRole('button', { name: 'Filter' }))
    const today = new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric', year: 'numeric' }).format(new Date())
    expect(screen.getByText('On')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: today })).toBeInTheDocument()
  })

  it('clearing a one-day filter applies no dates', () => {
    renderDashboard()
    applyTodayAsOneDay()

    fireEvent.click(screen.getByRole('button', { name: 'Filter' }))
    fireEvent.click(screen.getByRole('button', { name: /\d{4}$/ }))
    fireEvent.click(screen.getByRole('button', { name: 'Clear' }))
    fireEvent.click(screen.getByText(/^apply$/i))

    // Reopening reflects what was applied: an empty range, not the old day.
    fireEvent.click(screen.getByRole('button', { name: 'Filter' }))
    expect(screen.getByText('From')).toBeInTheDocument()
    expect(screen.getAllByRole('button', { name: /pick a date/i })).toHaveLength(2)
  })

  it('refetches with the applied preset, app and sort order', async () => {
    // Early in a month, so the last 7 days reach into the previous one.
    vi.useFakeTimers({ toFake: ['Date'] })
    vi.setSystemTime(new Date(2026, 2, 5, 12))
    mockBackend({ transcripts: sampleTranscripts })
    renderDashboard()
    await screen.findByText('Hello world')

    fireEvent.click(screen.getByRole('button', { name: 'Filter' }))
    fireEvent.click(screen.getByRole('button', { name: '7 days' }))
    fireEvent.click(await screen.findByRole('button', { name: 'App' }))
    // The list is portaled outside the filter panel; picking from it keeps the panel open.
    const slack = screen.getByRole('option', { name: 'Slack' })
    fireEvent.pointerDown(slack)
    fireEvent.click(slack)
    expect(screen.getByRole('button', { name: 'Filter' })).toHaveAttribute('aria-expanded', 'true')
    fireEvent.click(screen.getByText(/oldest first/i))
    fireEvent.click(screen.getByText(/^apply$/i))

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_transcripts', {
        page: expect.objectContaining({ from: '2026-02-27', to: '2026-03-05', app: 'Slack', sortAsc: true }),
      })
    })
    vi.useRealTimers()
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

  it('shows a new dictation when the next page is asked for as its reread starts', async () => {
    mockBackend({ transcripts: fullPage })
    renderDashboard()
    await waitFor(() => expect(observed.size).toBeGreaterThan(0))
    let finishReread: (rows: Transcript[]) => void = () => {}
    const base = mockInvoke.getMockImplementation()!
    mockInvoke.mockImplementation((cmd, args) => {
      if (cmd !== 'get_transcripts') return base(cmd, args)
      // The first page is the reread; a later page is the next one, and empty.
      const { page } = args as { page: { cursorId: number | null } }
      return page.cursorId === null ? new Promise(resolve => { finishReread = resolve }) : Promise.resolve([])
    })

    // The sentinel is still observed when the reread begins.
    await historyChanged()
    act(() => fireIntersect?.())
    await act(async () => { finishReread([{ ...fullPage[0], id: 99, content: 'Fresh dictation' }, ...fullPage]) })

    expect(await screen.findByText('Fresh dictation')).toBeInTheDocument()
  })

  it('renders a row once when the next page repeats it', async () => {
    let call = 0
    const base = mockInvoke.getMockImplementation()!
    mockInvoke.mockImplementation((cmd, args) =>
      cmd === 'get_transcripts' ? Promise.resolve(call++ === 0 ? fullPage : [fullPage[49]]) : base(cmd, args))
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

describe('Dashboard day groups', () => {
  it('keeps the day mounted when a new dictation joins it', async () => {
    mockBackend({ transcripts: sampleTranscripts })
    renderDashboard()
    const day = (await screen.findByText('Hello world')).closest('section')!
    const fresh: Transcript = { id: 3, content: 'Fresh dictation', wordCount: 2, durationSeconds: 1, targetApp: null, pinned: false, audioPath: null, sourceName: null, model: null, pending: false, revision: 0, createdAt: new Date().toISOString() }
    mockBackend({ transcripts: [fresh, ...sampleTranscripts] })
    await historyChanged()
    expect(await screen.findByText('Fresh dictation')).toBeInTheDocument()
    // Same node: the group was updated in place, not torn down and rebuilt.
    expect(screen.getByText('Hello world').closest('section')).toBe(day)
    expect(day.isConnected).toBe(true)
  })
})

describe('Dashboard day groups across years', () => {
  it('keeps the same weekday and date from different years apart', async () => {
    // Monday 8 Nov in both years: identical labels without the year.
    const at = (id: number, createdAt: string): Transcript => ({ id, content: `Entry ${id}`, wordCount: 2, durationSeconds: 1, targetApp: null, pinned: false, audioPath: null, sourceName: null, model: null, pending: false, revision: 0, createdAt })
    mockBackend({ transcripts: [at(1, '2021-11-08T12:00:00'), at(2, '2010-11-08T12:00:00')] })
    renderDashboard()
    const newer = (await screen.findByText('Entry 1')).closest('section')!
    const older = screen.getByText('Entry 2').closest('section')!
    expect(older).not.toBe(newer)
    expect(newer).toHaveTextContent('2021')
    expect(older).toHaveTextContent('2010')
  })
})
