import type { TranscriptFilters } from './transcripts'

export const queryKeys = {
  transcriptsRoot: ['transcripts'] as const,
  transcripts: (filters: TranscriptFilters) => ['transcripts', 'feed', filters] as const,
  transcriptSearch: (query: string, filters: TranscriptFilters) =>
    ['transcripts', 'search', query, filters] as const,
  stats: ['stats'] as const,
  dictionary: ['dictionary'] as const,
}
