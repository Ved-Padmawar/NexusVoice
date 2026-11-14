import { useQuery } from '@tanstack/react-query'
import { toast } from 'sonner'
import { commands, events, type JobFinish, type Previous } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { reads } from '../../app/sync'
import { queryKeys } from '../../shared/queryKeys'

/** The running transcription job, then the queued ones. */
const jobsOptions = {
  queryKey: queryKeys.jobs,
  queryFn: () => commands.getJobs(),
  ...reads('jobs'),
}

export function useSavedAudio() {
  return useQuery({
    queryKey: queryKeys.savedAudio,
    queryFn: () => commands.getSavedAudio(),
    ...reads('settings', 'savedAudio'),
  })
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

async function undoRetranscribe(id: number, previous: Previous, revision: number) {
  try {
    await commands.restoreTranscript(id, previous, revision)
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

/** Toast each job's end. */
export function listenForJobsFinishing() {
  return events.jobFinished.listen(({ payload }) => announce(payload))
}

/** Queues the job; the job list follows it. */
export async function startRetranscribe(id: number, model: string) {
  try {
    await commands.retranscribeTranscript(id, model)
  } catch (e) {
    toast.error(extractErrorMessage(e, 'Could not start the transcription'))
  }
}

export const cancelJob = (id: number) => commands.cancelJob(id)
