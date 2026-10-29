import { commands, events, type PillSettings, type PillSettingsSnapshot } from '../shared/bindings'

/**
 * Follow the pill's saved preferences: the current snapshot, then every
 * change. Listens before it fetches so no change slips between the two, and
 * drops anything older than what it already applied.
 */
export function subscribePillSettings(onChange: (settings: PillSettings, revision: number) => void): () => void {
  let revision = -1
  let stopped = false
  const apply = (snapshot: PillSettingsSnapshot) => {
    if (stopped || snapshot.revision <= revision) return
    revision = snapshot.revision
    onChange(snapshot.settings, snapshot.revision)
  }
  const unlisten = events.pillSettingsChanged.listen((e) => apply(e.payload))
  void unlisten.then(() => commands.getPillSettings()).then(apply).catch(() => {})
  return () => {
    stopped = true
    unlisten.then(fn => fn()).catch(() => {})
  }
}
