export type TranscriptFilters = {
  from: string | null
  to: string | null
  sortAsc: boolean
}

/** History reads end with the History revision they read at. */
export const queryKeys = {
  transcripts: (filters: TranscriptFilters) => ['transcripts', 'feed', filters] as const,
  transcriptSearch: (query: string, filters: TranscriptFilters) =>
    ['transcripts', 'search', query, filters] as const,
  stats: (rev: number) => ['stats', rev] as const,
  dailyWords: (days: number, rev: number) => ['stats', 'daily', days, rev] as const,
  catalog: ['catalog'] as const,
  languageOptionsFor: (model: string | null, settingsRev: number) => ['languageOptions', model, settingsRev] as const,
  inputDevices: ['inputDevices'] as const,
}
