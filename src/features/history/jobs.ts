import { toast } from 'sonner'
import { commands, events, type JobFinish, type Previous } from '../../shared/bindings'
import { extractErrorMessage } from '../../shared/errors'
import { useSlice } from '../../app/mirror'

/** Disk the saved audio takes, pinned entries' included. */
export function useSavedAudioBytes() {
  return useSlice('savedAudio', bytes => bytes)
}

/** The running transcription job, then the queued ones. */
export function useJobs() {
  return useSlice('jobs', jobs => jobs)
}

/** Whether a file is waiting or being transcribed. A boolean, so callers sleep through progress. */
export function useImporting() {
  return useSlice('jobs', jobs => jobs.some(job => job.kind === 'import'))
}

/** Whether entry `id` has a job. A boolean, so rows sleep through its progress. */
export function useHasJob(id: number) {
  return useSlice('jobs', jobs => jobs.some(job => job.transcriptId === id))
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
