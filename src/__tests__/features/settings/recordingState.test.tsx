import { describe, it, expect, vi, beforeEach } from 'vitest'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { queryClient } from '../../../app/queryClient'
import { listenForRecordingState, useKeptByRecording } from '../../../features/settings/queries'
import type { RecordingSnapshot } from '../../../shared/bindings'
import { queryKeys } from '../../../shared/queryKeys'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

const snapshot = (revision: number, deferred: RecordingSnapshot['deferred']): RecordingSnapshot =>
  ({ revision, status: deferred.length ? 'dictation' : 'idle', deferred })

let emit: (payload: RecordingSnapshot) => void

beforeEach(() => {
  queryClient.clear()
  vi.mocked(invoke).mockReset()
  vi.mocked(invoke).mockResolvedValue(snapshot(1, []))
  vi.mocked(listen).mockImplementation(async (_event, handler) => {
    emit = payload => (handler as (e: { payload: RecordingSnapshot }) => void)({ payload })
    return () => {}
  })
})

const wrapper = ({ children }: { children: ReactNode }) =>
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>

describe('recording state', () => {
  it('shows a kept input for as long as Rust says, and ignores a late older snapshot', async () => {
    await listenForRecordingState()
    const { result } = renderHook(() => useKeptByRecording('language'), { wrapper })
    await waitFor(() => expect(queryClient.getQueryData(queryKeys.recording)).toBeDefined())
    expect(result.current).toBe(false)

    act(() => emit(snapshot(3, ['language'])))
    await waitFor(() => expect(result.current).toBe(true))

    // Delivered late: an older snapshot never brings back what it said.
    act(() => emit(snapshot(2, [])))
    expect(queryClient.getQueryData(queryKeys.recording)).toMatchObject({ revision: 3 })
    await act(() => new Promise(r => setTimeout(r, 20)))
    expect(result.current).toBe(true)

    // The recording finished and the change applied.
    act(() => emit(snapshot(4, [])))
    await waitFor(() => expect(result.current).toBe(false))
  })
})
