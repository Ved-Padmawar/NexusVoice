export type TranscriptFilters = {
  from: string | null
  to: string | null
  sortAsc: boolean
}

export const queryKeys = {
  transcripts: (filters: TranscriptFilters) => ['transcripts', 'feed', filters] as const,
  transcriptSearch: (query: string, filters: TranscriptFilters) =>
    ['transcripts', 'search', query, filters] as const,
  stats: ['stats'] as const,
  dailyWords: (days: number) => ['stats', 'daily', days] as const,
  dictionary: ['dictionary'] as const,
  catalog: ['catalog'] as const,
  languageOptionsFor: (model: string | null, settingsRev: number) => ['languageOptions', model, settingsRev] as const,
  inputDevices: ['inputDevices'] as const,
  savedAudio: ['savedAudio'] as const,
  jobs: ['jobs'] as const,
}
