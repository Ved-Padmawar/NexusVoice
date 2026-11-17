export type TranscriptFilters = {
  from: string | null
  to: string | null
  sortAsc: boolean
}

export const hasDates = ({ from, to }: TranscriptFilters) => !!from || !!to
export const isFiltered = (filters: TranscriptFilters) => hasDates(filters) || filters.sortAsc

/** History reads end with the History revision they read at. */
export const queryKeys = {
  transcripts: (filters: TranscriptFilters) => ['transcripts', 'feed', filters] as const,
  transcriptSearch: (query: string, filters: TranscriptFilters) =>
    ['transcripts', 'search', query, filters] as const,
  stats: (rev: number) => ['stats', rev] as const,
  dailyWords: (days: number, localDate: string, utcOffset: number, rev: number) => ['stats', 'daily', days, localDate, utcOffset, rev] as const,
  catalog: ['catalog'] as const,
  inputDevices: ['inputDevices'] as const,
}
