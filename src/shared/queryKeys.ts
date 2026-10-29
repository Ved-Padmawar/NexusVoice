export type TranscriptFilters = {
  from: string | null
  to: string | null
  sortAsc: boolean
}

export const queryKeys = {
  transcriptsRoot: ['transcripts'] as const,
  transcripts: (filters: TranscriptFilters) => ['transcripts', 'feed', filters] as const,
  transcriptSearch: (query: string, filters: TranscriptFilters) =>
    ['transcripts', 'search', query, filters] as const,
  stats: ['stats'] as const,
  // Under `stats`, so whatever refreshes the totals refreshes this too.
  dailyWords: (days: number) => ['stats', 'daily', days] as const,
  dictionary: ['dictionary'] as const,
  catalog: ['catalog'] as const,
  activeModel: ['activeModel'] as const,
  downloads: ['downloads'] as const,
  hotkeys: ['hotkeys'] as const,
  pillSettings: ['pillSettings'] as const,
  cuda: ['cuda'] as const,
}
