import { describe, it, expect, vi, beforeEach } from 'vitest'
import { act, renderHook, waitFor } from '@testing-library/react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useKeptByRecording } from '../../../features/settings/queries'
import type { RecordingSnapshot } from '../../../shared/bindings'

const snapshot = (revision: number, deferred: RecordingSnapshot['deferred']): RecordingSnapshot =>
  ({ revision, status: deferred.length ? 'dictation' : 'idle', deferred })

let emit: (payload: RecordingSnapshot) => void

beforeEach(() => {
  vi.mocked(invoke).mockReset()
  vi.mocked(invoke).mockResolvedValue(snapshot(1, []))
  vi.mocked(listen).mockImplementation(async (_event, handler) => {
    emit = payload => (handler as (e: { payload: RecordingSnapshot }) => void)({ payload })
    return () => {}
  })
})

describe('recording state', () => {
  it('shows a kept input for as long as Rust says, and ignores a late older snapshot', async () => {
    const { result } = renderHook(() => useKeptByRecording('language'))
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('get_recording_state'))
    expect(result.current).toBe(false)

    act(() => emit(snapshot(3, ['language'])))
    await waitFor(() => expect(result.current).toBe(true))

    // Delivered late: an older snapshot never brings back what it said.
    act(() => emit(snapshot(2, [])))
    await act(() => new Promise(r => setTimeout(r, 20)))
    expect(result.current).toBe(true)

    // The recording finished and the change applied.
    act(() => emit(snapshot(4, [])))
    await waitFor(() => expect(result.current).toBe(false))
  })
})
