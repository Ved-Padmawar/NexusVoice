import { useQuery } from '@tanstack/react-query'
import { toast } from 'sonner'
import { commands, events, type JobFinish, type JobSnapshot, type Previous } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { live, patchLive, queryClient, readFresh } from '../../app/queryClient'
import { queryKeys } from '../../shared/queryKeys'
import type { Transcript } from '../../shared/types'
import { refreshDictionaryUses } from '../dictionary/queries'
import { dropRow, patchRows, showRewritten } from './queries'

/** The running transcription job, then the queued ones. */
export const jobsOptions = live(queryKeys.jobs, () => commands.getJobs())

export const savedAudioOptions = live(queryKeys.savedAudio, () => commands.getSavedAudio())

export function useSavedAudio() {
  return useQuery(savedAudioOptions)
}

export function useJobs() {
  return useQuery(jobsOptions).data ?? []
}

/** Whether a file is waiting or being transcribed. A boolean, so callers sleep through progress. */
export function useImporting() {
  return useQuery({ ...jobsOptions, select: jobs => jobs.some(job => job.kind === 'import') }).data ?? false
}

/** Whether entry `id` has a job. A boolean, so rows sleep through its progress. */
export function useHasJob(id: number) {
  return useQuery({ ...jobsOptions, select: jobs => jobs.some(job => job.transcriptId === id) }).data ?? false
}

/** Models on disk, read fresh each time a picker opens: a download may have finished. */
export function useDownloadedModels(enabled = true) {
  return useQuery({
    queryKey: queryKeys.downloadedModels,
    queryFn: () => commands.getDownloadedModels(),
    staleTime: 0,
    enabled,
  })
}

const withJob = (jobs: JobSnapshot[], job: JobSnapshot) =>
  jobs.some(j => j.transcriptId === job.transcriptId)
    ? jobs.map(j => (j.transcriptId === job.transcriptId ? job : j))
    : [...jobs, job]

async function undoRetranscribe(id: number, previous: Previous, revision: number) {
  try {
    await showRewritten(await commands.restoreTranscript(id, previous, revision))
  } catch (e) {
    toast.error(extractErrorMessage(e, 'Could not restore the previous text'))
  }
}

function announce({ outcome, kind, transcriptId, previous, entry, message }: JobFinish) {
  if (outcome === 'failed') {
    toast.error(message ?? 'The transcription failed')
    return
  }
  if (outcome !== 'done' || !entry) return
  if (kind === 'import') {
    toast.success('File transcribed', {
      description: entry.sourceName ?? undefined,
      action: { label: 'Copy', onClick: () => void navigator.clipboard.writeText(entry.content) },
    })
    return
  }
  toast.success('Transcript updated', {
    action: previous && { label: 'Undo', onClick: () => void undoRetranscribe(transcriptId, previous, entry.revision) },
  })
}

/** Keep the jobs, saved audio and the feed in step with the backend's events. */
export async function listenForAudioEvents(): Promise<() => void> {
  const unlisteners = await Promise.all([
    events.jobUpdated.listen(({ payload }) => patchLive(jobsOptions, jobs => withJob(jobs, payload))),
    events.jobFinished.listen(({ payload }) => {
      patchLive(jobsOptions, jobs => jobs.filter(job => job.transcriptId !== payload.transcriptId))
      announce(payload)
      if (payload.entry) {
        void showRewritten(payload.entry)
        if (payload.kind === 'import') void refreshDictionaryUses()
      } else if (payload.kind === 'import') {
        // An import that didn't finish takes its entry, and its count in the totals, with it.
        void dropRow(payload.transcriptId)
        void queryClient.invalidateQueries({ queryKey: queryKeys.stats, refetchType: document.hidden ? 'none' : 'active' })
      }
    }),
    events.savedAudioChanged.listen(({ payload }) => {
      readFresh(savedAudioOptions).catch(() => {})
      void patchRows(new Map(payload.map(({ id, audioPath }) => [id, (row: Transcript) => ({ ...row, audioPath })])))
    }),
  ])
  return () => unlisteners.forEach(fn => fn())
}

/** Queues the job; its progress arrives as events. */
export async function startRetranscribe(id: number, model: string) {
  try {
    await commands.retranscribeTranscript(id, model)
  } catch (e) {
    toast.error(extractErrorMessage(e, 'Could not start the transcription'))
  }
}

export const cancelJob = (id: number) => commands.cancelJob(id)
