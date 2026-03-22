import { describe, it, expect, vi, beforeEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore } from '../store/useAppStore'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }))

const mockInvoke = vi.mocked(invoke)

beforeEach(() => {
  mockInvoke.mockReset()
  useAppStore.setState({
    user: null,
    refreshToken: null,
    transcripts: [],
    transcriptOffset: 0,
    transcriptHasMore: true,
    filterFrom: null,
    filterTo: null,
    filterSortAsc: false,
    searchQuery: '',
    searchResults: [],
    isSearching: false,
    dictionary: [],
    stats: null,
    isLoading: false,
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
  it('sets user on success with rememberMe=true', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'login_with_tokens') return Promise.resolve({
        user: { id: 1, email: 'test@example.com' },
        tokens: { accessToken: 'access', refreshToken: 'refresh', expiresInSeconds: 3600 },
      })
      return Promise.resolve(undefined)
    })

    await useAppStore.getState().login('test@example.com', 'password', true)
    const state = useAppStore.getState()
    expect(state.user?.email).toBe('test@example.com')
    expect(state.refreshToken).toBe('refresh')
  })

  it('does not store refreshToken when rememberMe=false', async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'login_with_tokens') return Promise.resolve({
        user: { id: 1, email: 'test@example.com' },
        tokens: { accessToken: 'access', refreshToken: 'refresh', expiresInSeconds: 3600 },
      })
      return Promise.resolve(undefined)
    })

    await useAppStore.getState().login('test@example.com', 'password', false)
    expect(useAppStore.getState().refreshToken).toBeNull()
  })

  it('throws on failed login', async () => {
    mockInvoke.mockRejectedValue({ message: 'Invalid credentials' })
    await expect(useAppStore.getState().login('bad@example.com', 'wrong', false)).rejects.toThrow()
  })
})

describe('useAppStore — logout', () => {
  it('clears user, tokens and data', async () => {
    useAppStore.setState({
      user: { id: 1, email: 'test@example.com' },
      refreshToken: 'token',
      transcripts: [{ id: 1, content: 'hello', wordCount: 1, durationSeconds: null, createdAt: '' }],
    })
    mockInvoke.mockResolvedValue(undefined)
    await useAppStore.getState().logout()
    const state = useAppStore.getState()
    expect(state.user).toBeNull()
    expect(state.refreshToken).toBeNull()
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
  it('appends transcripts and updates offset', async () => {
    const batch = Array.from({ length: 3 }, (_, i) => ({ id: i + 1, content: `t${i}`, wordCount: 1, durationSeconds: null, createdAt: '' }))
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_transcripts') return Promise.resolve(batch)
      return Promise.resolve(undefined)
    })
    await useAppStore.getState().loadMoreTranscripts()
    expect(useAppStore.getState().transcripts).toHaveLength(3)
    expect(useAppStore.getState().transcriptOffset).toBe(3)
    expect(useAppStore.getState().transcriptHasMore).toBe(false)
  })

  it('does nothing if hasMore is false', async () => {
    useAppStore.setState({ transcriptHasMore: false })
    await useAppStore.getState().loadMoreTranscripts()
    expect(mockInvoke).not.toHaveBeenCalled()
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
  it('resets offset and fetches with new filters', async () => {
    const items = [{ id: 1, content: 'hello', wordCount: 1, durationSeconds: null, createdAt: '2026-01-01' }]
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_transcripts') return Promise.resolve(items)
      return Promise.resolve(undefined)
    })
    await useAppStore.getState().setFilters('2026-01-01', '2026-01-31', false)
    expect(useAppStore.getState().filterFrom).toBe('2026-01-01')
    expect(useAppStore.getState().filterTo).toBe('2026-01-31')
    expect(useAppStore.getState().transcripts).toHaveLength(1)
    expect(useAppStore.getState().transcriptOffset).toBe(1)
  })

  it('resets filters when called with nulls', async () => {
    mockInvoke.mockResolvedValue([])
    await useAppStore.getState().setFilters(null, null, false)
    expect(useAppStore.getState().filterFrom).toBeNull()
    expect(useAppStore.getState().filterTo).toBeNull()
  })
})
