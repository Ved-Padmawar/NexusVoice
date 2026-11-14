import { focusManager, type Query, type QueryKey } from '@tanstack/react-query'
import { events, type Topic } from '../shared/bindings'
import { queryClient } from './queryClient'

declare module '@tanstack/react-query' {
  interface Register {
    queryMeta: { reads?: readonly Topic[] }
  }
}

/** A failed read retries twice, soon, so the change it was for never waits
 *  on the next one. */
const RETRIES = 2
const retryDelay = (failures: number) => 100 * 2 ** failures

/** Options for a query of Rust data, naming every owner its command reads. It
 *  reads again when one announces a change, or, if the window was hidden then,
 *  once the window is shown. */
export const reads = (...topics: Topic[]) =>
  ({ meta: { reads: topics }, staleTime: Infinity, refetchOnWindowFocus: true, retry: RETRIES, retryDelay }) as const

const readsTopic = (topic: Topic) => (query: Query) => query.meta?.reads?.includes(topic) === true

const fetching = (query: Query) => query.state.fetchStatus === 'fetching'

// Shown again or focused: the pill never takes focus, and switching from
// another window, such as the other build's, changes no visibility.
focusManager.setEventListener((onFocus) => {
  const check = () => onFocus()
  window.addEventListener('visibilitychange', check)
  window.addEventListener('focus', check)
  return () => {
    window.removeEventListener('visibilitychange', check)
    window.removeEventListener('focus', check)
  }
})

/** Mark `queries` stale, reading those on screen now unless the window is hidden. */
function reread(queries: ReadonlySet<Query>) {
  if (queries.size === 0) return
  void queryClient.invalidateQueries({
    predicate: query => queries.has(query),
    refetchType: document.hidden ? 'none' : 'active',
  })
}

/** Reads that began before a change: each lands, then reads again. Not
 *  cancelled, or a burst of changes would show nothing until it ended. */
const rereadOnLanding = new Set<Query>()

queryClient.getQueryCache().subscribe((event) => {
  if (!rereadOnLanding.has(event.query)) return
  if (event.type === 'removed') rereadOnLanding.delete(event.query)
  else if (event.type === 'updated' && !fetching(event.query)) {
    rereadOnLanding.delete(event.query)
    reread(new Set([event.query]))
  }
})

/** Read `topic` again; a read in flight lands first. */
function changed(topic: Topic) {
  const now = new Set<Query>()
  for (const query of queryClient.getQueryCache().findAll({ predicate: readsTopic(topic) })) {
    if (fetching(query)) rereadOnLanding.add(query)
    else now.add(query)
  }
  reread(now)
}

/** Follow Rust's announcements. Awaited before the first render, so no
 *  change can land between a read and the listener. */
export async function followRust() {
  await events.changed.listen(({ payload }) => changed(payload))
}

/** Read `query` now. One in flight with data is replaced, and whoever awaits
 *  it gets this read; a first read in flight is cancelled, since a refetch
 *  would join it. Rejects if this read fails after its retries. */
async function readNow(query: Query) {
  const self = { predicate: (other: Query) => other === query }
  if (fetching(query) && query.state.data === undefined) await queryClient.cancelQueries(self)
  await queryClient.refetchQueries(self, { throwOnError: true })
}

/** Resolves once every `topic` query on screen holds a read started after this
 *  call, or one failed even after its retries. A save awaits it, so the
 *  screen doesn't flash the value from before the save. */
export async function caughtUp(topic: Topic) {
  const matches = readsTopic(topic)
  const onScreen = queryClient.getQueryCache().findAll({ predicate: query => matches(query) && query.isActive() })
  await Promise.all(onScreen.map(query => readNow(query).catch(() => {})))
}

/** Resolves once `queryKey` holds a successful read started after this call:
 *  this one, or, if it fails, any later one. */
export function readAfresh(queryKey: QueryKey) {
  const query = queryClient.getQueryCache().find({ queryKey, exact: true })
  if (!query) return Promise.resolve()
  return new Promise<void>((resolve) => {
    let startedSince = false
    const done = () => {
      stop()
      resolve()
    }
    const stop = queryClient.getQueryCache().subscribe((event) => {
      if (event.query !== query) return
      if (event.type === 'removed') done()
      else if (event.type === 'updated' && event.action.type === 'fetch') startedSince = true
      else if (event.type === 'updated' && event.action.type === 'success' && startedSince) done()
    })
    readNow(query).then(done, () => {})
  })
}
