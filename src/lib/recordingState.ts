import { commands, events, type RecordingSnapshot } from '../bindings'

/** Listen before reading the snapshot; revisions reject late IPC replies. */
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
