import { useEffect, useState } from 'react'
import { commands, events, type RecordingSnapshot } from '../shared/bindings'

/**
 * Follow every transition of the recording. It is a stream, not cached data:
 * each snapshot carries a revision, so a late IPC reply never replaces a
 * newer one. Listens before reading.
 */
export function subscribeRecordingState(onChange: (snapshot: RecordingSnapshot) => void): () => void {
  let revision = -1
  let stopped = false
  const apply = (snapshot: RecordingSnapshot) => {
    if (stopped || snapshot.revision <= revision) return
    revision = snapshot.revision
    onChange(snapshot)
  }
  const unlisten = events.recordingStateChanged.listen(event => apply(event.payload))
  void unlisten.then(() => commands.getRecordingState()).then(apply).catch(() => {})
  return () => {
    stopped = true
    void unlisten.then(fn => fn()).catch(() => {})
  }
}

/** The recording's latest snapshot; null until the first one arrives. */
export function useRecordingState() {
  const [snapshot, setSnapshot] = useState<RecordingSnapshot | null>(null)
  useEffect(() => subscribeRecordingState(setSnapshot), [])
  return snapshot
}
