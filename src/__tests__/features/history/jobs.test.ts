import { describe, it, expect, vi, beforeEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { queryClient } from '../../../app/queryClient'
import { listenForAudioEvents } from '../../../features/history/jobs'
import { addTranscript, NO_FILTERS, type TranscriptPages } from '../../../features/history/queries'
import type { JobFinish, JobSnapshot } from '../../../shared/bindings'
import { queryKeys } from '../../../shared/queryKeys'
import type { Transcript } from '../../../shared/types'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))
vi.mock('sonner', () => ({ toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn() }) }))

const handlers = new Map<string, (event: { payload: unknown }) => void>()
const entry = (id: number, pending: boolean): Transcript => ({ id, content: pending ? '' : `Entry ${id}`, wordCount: 2, durationSeconds: 1, targetApp: null, pinned: false, audioPath: null, sourceName: 'talk.mp3', model: null, pending, revision: 0, createdAt: '2026-11-14 10:00:00' })
const job = (transcriptId: number): JobSnapshot => ({ transcriptId, kind: 'import', label: 'talk.mp3', model: 'Whisper Small', queued: false, progress: 0.5, paused: false })
const feedKey = queryKeys.transcripts(NO_FILTERS)

beforeEach(() => {
  queryClient.clear()
  handlers.clear()
  vi.mocked(invoke).mockReset().mockResolvedValue(undefined)
  vi.mocked(listen).mockImplementation(async (event, handler) => {
    handlers.set(event, handler as (event: { payload: unknown }) => void)
    return () => {}
  })
})

describe('job events', () => {
  it('fills in an import whose job finishes while its entry is still landing', async () => {
    await listenForAudioEvents()
    queryClient.setQueryData<TranscriptPages>(feedKey, { pages: [{ rows: [entry(4, false)] }], pageParams: [{ cursorCreatedAt: null, cursorId: null }] })
    queryClient.setQueryData<JobSnapshot[]>(queryKeys.jobs, [job(5)])
    const done = entry(5, false)
    const finish: JobFinish = { transcriptId: 5, kind: 'import', outcome: 'done', previous: null, entry: done, message: null }

    void addTranscript(entry(5, true))
    handlers.get('job-finished')?.({ payload: finish })

    await vi.waitFor(() => expect(queryClient.getQueryData<TranscriptPages>(feedKey)?.pages[0].rows).toEqual([done, entry(4, false)]))
    expect(queryClient.getQueryData(queryKeys.jobs)).toEqual([])
  })

})
