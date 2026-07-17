import { describe, it, expect, vi, beforeEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore } from '../../store/useAppStore'
import { PAGE_SIZE } from '../../store/transcriptSlice'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }))
vi.mock('sonner', () => ({ toast: { error: vi.fn(), success: vi.fn() } }))

const mockInvoke = vi.mocked(invoke)

const page = (start: number, count: number) =>
  Array.from({ length: count }, (_, i) => ({
    id: start + i + 1,
    content: `t${start + i}`,
    wordCount: 1,
    durationSeconds: null,
    createdAt: `2026-01-01T00:00:${String(start + i + 1).padStart(2, '0')}`,
  }))

beforeEach(() => {
  mockInvoke.mockReset()
  useAppStore.setState({
    user: null,
    transcripts: [],
    transcriptHasMore: true,
    transcriptLoadingMore: false,
    filterFrom: null,
    filterTo: null,
    filterSortAsc: false,
    searchQuery: '',
    searchResults: [],
    isSearching: false,
    dictionary: [],
    stats: null,
    transcriptsStatus: 'idle',
    transcriptsError: null,
    dictionaryStatus: 'idle',
    dictionaryError: null,
    statsStatus: 'idle',
    statsError: null,
    authChecking: false,
    hasHotkey: false,
    modelReady: false,
    modelDownloading: false,
    downloadProgress: 0,
    downloadError: null,
    updateAvailable: null,
  })
})

describe('useAppStore — theme', () => {
  it('setTheme updates theme', () => {
    useAppStore.getState().setTheme('midnight')
    expect(useAppStore.getState().theme).toBe('midnight')
  })
})

describe('useAppStore — login', () => {
  it('sets user on success', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'login') return Promise.resolve({ id: 1, email: 'test@example.com' })
      return Promise.resolve(undefined)
    })

    await useAppStore.getState().login('test@example.com', 'password')
    const state = useAppStore.getState()
    expect(state.user?.email).toBe('test@example.com')
  })

  it('calls the login command with email and password', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'login') return Promise.resolve({ id: 1, email: 'test@example.com' })
      return Promise.resolve(undefined)
    })

    await useAppStore.getState().login('test@example.com', 'password')
    expect(mockInvoke).toHaveBeenCalledWith('login', { email: 'test@example.com', password: 'password' })
  })

  it('throws on failed login', async () => {
    mockInvoke.mockRejectedValue({ message: 'Invalid credentials' })
    await expect(useAppStore.getState().login('bad@example.com', 'wrong')).rejects.toThrow()
  })
})

describe('useAppStore — logout', () => {
  it('clears user and data', async () => {
    useAppStore.setState({
      user: { id: 1, email: 'test@example.com' },
      transcripts: [{ id: 1, content: 'hello', wordCount: 1, durationSeconds: null, createdAt: '' }],
    })
    mockInvoke.mockResolvedValue(undefined)
    await useAppStore.getState().logout()
    const state = useAppStore.getState()
    expect(state.user).toBeNull()
    expect(state.transcripts).toHaveLength(0)
  })
})

describe('useAppStore — searchTranscripts', () => {
  it('sets searchResults on success', async () => {
    const results = [{ id: 1, content: 'hello world', wordCount: 2, durationSeconds: null, createdAt: '' }]
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'search_transcripts') return Promise.resolve(results)
      return Promise.resolve(undefined)
    })
    await useAppStore.getState().searchTranscripts('hello')
    expect(useAppStore.getState().searchResults).toHaveLength(1)
    expect(useAppStore.getState().isSearching).toBe(false)
  })

  it('clears results on empty query', async () => {
    useAppStore.setState({ searchResults: [{ id: 1, content: 'x', wordCount: 1, durationSeconds: null, createdAt: '' }] })
    await useAppStore.getState().searchTranscripts('')
    expect(useAppStore.getState().searchResults).toHaveLength(0)
  })
})

describe('useAppStore — loadMoreTranscripts', () => {
  it('appends a short page and marks the end', async () => {
    mockInvoke.mockImplementation((cmd) => cmd === 'get_transcripts' ? Promise.resolve(page(0, 3)) : Promise.resolve(undefined))
    await useAppStore.getState().loadMoreTranscripts()
    expect(useAppStore.getState().transcripts).toHaveLength(3)
    expect(useAppStore.getState().transcriptHasMore).toBe(false)
  })

  it('does nothing if hasMore is false', async () => {
    useAppStore.setState({ transcriptHasMore: false })
    await useAppStore.getState().loadMoreTranscripts()
    expect(mockInvoke).not.toHaveBeenCalled()
  })

  it('keeps hasMore true on a full page', async () => {
    mockInvoke.mockImplementation((cmd) => cmd === 'get_transcripts' ? Promise.resolve(page(0, PAGE_SIZE)) : Promise.resolve(undefined))
    await useAppStore.getState().loadMoreTranscripts()
    expect(useAppStore.getState().transcriptHasMore).toBe(true)
  })

  it('sends the last loaded row as the cursor', async () => {
    useAppStore.setState({ transcripts: page(0, 3) })
    mockInvoke.mockImplementation((cmd) => cmd === 'get_transcripts' ? Promise.resolve([]) : Promise.resolve(undefined))
    await useAppStore.getState().loadMoreTranscripts()
    expect(mockInvoke).toHaveBeenCalledWith('get_transcripts', {
      page: expect.objectContaining({ cursorCreatedAt: '2026-01-01T00:00:03', cursorId: 3 }),
    })
  })

  it('sends a null cursor when the feed is empty', async () => {
    mockInvoke.mockImplementation((cmd) => cmd === 'get_transcripts' ? Promise.resolve([]) : Promise.resolve(undefined))
    await useAppStore.getState().loadMoreTranscripts()
    expect(mockInvoke).toHaveBeenCalledWith('get_transcripts', {
      page: expect.objectContaining({ cursorCreatedAt: null, cursorId: null }),
    })
  })

  // The stuck-spinner bug: the sentinel refires while visible, so concurrent calls
  // all fetch the same page and hasMore never settles.
  it('ignores overlapping calls while a fetch is in flight', async () => {
    useAppStore.setState({ transcripts: page(0, PAGE_SIZE) })
    let calls = 0
    mockInvoke.mockImplementation((cmd) => {
      if (cmd !== 'get_transcripts') return Promise.resolve(undefined)
      calls++
      return new Promise(resolve => setTimeout(() => resolve(page(PAGE_SIZE, 5)), 10))
    })

    await Promise.all([
      useAppStore.getState().loadMoreTranscripts(),
      useAppStore.getState().loadMoreTranscripts(),
      useAppStore.getState().loadMoreTranscripts(),
    ])

    expect(calls).toBe(1)
    expect(useAppStore.getState().transcripts).toHaveLength(PAGE_SIZE + 5)
    expect(useAppStore.getState().transcriptHasMore).toBe(false)
    expect(useAppStore.getState().transcriptLoadingMore).toBe(false)
  })

  it('does not append rows already in the feed', async () => {
    useAppStore.setState({ transcripts: page(0, 3) })
    mockInvoke.mockImplementation((cmd) => cmd === 'get_transcripts' ? Promise.resolve(page(2, 3)) : Promise.resolve(undefined))
    await useAppStore.getState().loadMoreTranscripts()
    const ids = useAppStore.getState().transcripts.map(t => t.id)
    expect(new Set(ids).size).toBe(ids.length)
  })

  it('clears the in-flight flag on failure so the next scroll retries', async () => {
    mockInvoke.mockRejectedValue({ message: 'boom' })
    await useAppStore.getState().loadMoreTranscripts()
    expect(useAppStore.getState().transcriptLoadingMore).toBe(false)
    expect(useAppStore.getState().transcripts).toHaveLength(0)
  })
})

describe('useAppStore — addTranscript', () => {
  it('prepends new transcript to list', async () => {
    const newT = { id: 99, content: 'new one', wordCount: 2, durationSeconds: null, createdAt: '' }
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'save_transcript') return Promise.resolve(newT)
      return Promise.resolve(undefined)
    })
    await useAppStore.getState().addTranscript('new one')
    const transcripts = useAppStore.getState().transcripts
    expect(transcripts[0].id).toBe(99)
  })
})

describe('useAppStore — updateDictionary', () => {
  it('adds new entry to dictionary', async () => {
    const entry = { id: 1, term: 'teh', replacement: 'the', hits: 0, createdAt: '' }
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'update_dictionary') return Promise.resolve(entry)
      return Promise.resolve(undefined)
    })
    await useAppStore.getState().updateDictionary('teh', 'the')
    expect(useAppStore.getState().dictionary[0].term).toBe('teh')
  })

  it('updates existing entry in place', async () => {
    const existing = { id: 1, term: 'teh', replacement: 'the', hits: 2, createdAt: '' }
    useAppStore.setState({ dictionary: [existing] })
    const updated = { ...existing, replacement: 'THE' }
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'update_dictionary') return Promise.resolve(updated)
      return Promise.resolve(undefined)
    })
    await useAppStore.getState().updateDictionary('teh', 'THE')
    expect(useAppStore.getState().dictionary[0].replacement).toBe('THE')
    expect(useAppStore.getState().dictionary).toHaveLength(1)
  })
})

describe('useAppStore — deleteDictionaryEntry', () => {
  it('removes entry from dictionary', async () => {
    useAppStore.setState({ dictionary: [{ id: 1, term: 'teh', replacement: 'the', hits: 0, createdAt: '' }] })
    mockInvoke.mockResolvedValue(undefined)
    await useAppStore.getState().deleteDictionaryEntry(1)
    expect(useAppStore.getState().dictionary).toHaveLength(0)
  })
})

describe('useAppStore — setFilters', () => {
  it('restarts the feed from a null cursor with new filters', async () => {
    mockInvoke.mockImplementation((cmd) => cmd === 'get_transcripts' ? Promise.resolve(page(0, 1)) : Promise.resolve(undefined))
    await useAppStore.getState().setFilters('2026-01-01', '2026-01-31', false)
    expect(useAppStore.getState().filterFrom).toBe('2026-01-01')
    expect(useAppStore.getState().filterTo).toBe('2026-01-31')
    expect(useAppStore.getState().transcripts).toHaveLength(1)
    expect(mockInvoke).toHaveBeenCalledWith('get_transcripts', {
      page: expect.objectContaining({ cursorCreatedAt: null, cursorId: null, from: '2026-01-01' }),
    })
  })

  it('resets filters when called with nulls', async () => {
    mockInvoke.mockResolvedValue([])
    await useAppStore.getState().setFilters(null, null, false)
    expect(useAppStore.getState().filterFrom).toBeNull()
    expect(useAppStore.getState().filterTo).toBeNull()
  })
})
