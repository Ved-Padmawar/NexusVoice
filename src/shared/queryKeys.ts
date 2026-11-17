export type TranscriptFilters = {
  from: string | null
  to: string | null
  app: string | null
  sortAsc: boolean
}

/** Filters that leave rows out, which sorting does not. */
export const isNarrowed = ({ from, to, app }: TranscriptFilters) => !!from || !!to || !!app
export const isFiltered = (filters: TranscriptFilters) => isNarrowed(filters) || filters.sortAsc

/** History reads end with the History revision they read at. */
export const queryKeys = {
  transcripts: (filters: TranscriptFilters) => ['transcripts', 'feed', filters] as const,
  transcriptSearch: (query: string, filters: TranscriptFilters) =>
    ['transcripts', 'search', query, filters] as const,
  stats: (rev: number) => ['stats', rev] as const,
  dailyWords: (days: number, localDate: string, utcOffset: number, rev: number) => ['stats', 'daily', days, localDate, utcOffset, rev] as const,
  historyApps: (rev: number) => ['historyApps', rev] as const,
  catalog: ['catalog'] as const,
  inputDevices: ['inputDevices'] as const,
}
