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
  activeModel: ['activeModel'] as const,
  downloads: ['downloads'] as const,
  downloadedModels: ['downloadedModels'] as const,
  languageOptions: ['languageOptions'] as const,
  languageOptionsFor: (model: string | null) => ['languageOptions', model] as const,
  modelUnload: ['modelUnload'] as const,
  pasteMode: ['pasteMode'] as const,
  keepMicOpen: ['keepMicOpen'] as const,
  inputDevices: ['inputDevices'] as const,
  hotkeys: ['hotkeys'] as const,
  boundHotkeys: ['boundHotkeys'] as const,
  pillSettings: ['pillSettings'] as const,
  formatConfig: ['formatConfig'] as const,
  cuda: ['cuda'] as const,
  savedAudio: ['savedAudio'] as const,
  jobs: ['jobs'] as const,
}
