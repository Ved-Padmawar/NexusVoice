import { QueryClient, hashKey } from '@tanstack/react-query'

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // These queries use local Tauri IPC, even when the PC is offline.
      networkMode: 'always',
      staleTime: 30_000,
      refetchOnWindowFocus: false,
      retry: false,
    },
    mutations: { networkMode: 'always' },
  },
})

/** Backend state that Rust's events keep current for the app's lifetime. */
export const LIVE = { staleTime: Infinity, gcTime: Infinity } as const

/** An event's change. It sets fields rather than adding to them, so applying it twice is harmless. */
type Patch<T> = (data: T) => T

/** Per query: the newest read, and the events each read in flight has missed. */
type Track = { latestRead: number; missed: Set<Patch<unknown>[]> }

const tracks = new Map<string, Track>()

function trackOf(queryKey: readonly unknown[]): Track {
  const hash = hashKey(queryKey)
  let track = tracks.get(hash)
  if (!track) {
    track = { latestRead: 0, missed: new Set() }
    tracks.set(hash, track)
  }
  return track
}

/**
 * Every read of a live query goes through here, whoever starts it. The events
 * that land during a read are replayed onto its result; `latest` is false once
 * a newer read has begun, and that one's result is the one to keep.
 */
async function trackedRead<T>(queryKey: readonly unknown[], read: () => Promise<T>) {
  const track = trackOf(queryKey)
  const sequence = ++track.latestRead
  const missed: Patch<unknown>[] = []
  track.missed.add(missed)
  try {
    const data = missed.reduce<unknown>((current, patch) => patch(current), await read()) as T
    return { data, latest: sequence === track.latestRead }
  } finally {
    track.missed.delete(missed)
  }
}

export type Live<T> = ReturnType<typeof live<T>>

/** Query options for backend state that Rust's events keep current. */
export function live<T>(queryKey: readonly unknown[], read: () => Promise<T>) {
  return {
    queryKey,
    read,
    queryFn: async () => {
      const { data, latest } = await trackedRead(queryKey, read)
      return latest ? data : queryClient.getQueryData<T>(queryKey) ?? data
    },
    ...LIVE,
  }
}

/** Read backend state after it changed. A cached read in flight may predate the
 * change, so it is cancelled first. */
export async function readFresh<T>({ queryKey, read }: Live<T>): Promise<T> {
  const { data, latest } = await trackedRead(queryKey, async () => {
    await queryClient.cancelQueries({ queryKey, exact: true })
    return read()
  })
  if (latest) queryClient.setQueryData(queryKey, data)
  return data
}

/** Apply an event to the cached copy, or read the state fresh when there is none yet. */
export function patchLive<T>(options: Live<T>, patch: Patch<T>) {
  const track = trackOf(options.queryKey)
  track.missed.forEach(missed => missed.push(patch as Patch<unknown>))
  if (queryClient.getQueryData<T>(options.queryKey) !== undefined) {
    queryClient.setQueryData<T>(options.queryKey, data => data && patch(data))
  } else if (track.missed.size === 0) {
    readFresh(options).catch(() => {})
  }
}
