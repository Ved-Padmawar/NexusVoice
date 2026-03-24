/**
 * Centralized Tauri event name constants.
 * Use these instead of raw string literals so renames are caught at compile time.
 */
export const EVENTS = {
  // Model download lifecycle
  MODEL_DOWNLOAD_START:     'model-download-start',
  MODEL_DOWNLOAD_PROGRESS:  'model-download-progress',
  MODEL_DOWNLOAD_COMPLETE:  'model-download-complete',
  MODEL_DOWNLOAD_ERROR:     'model-download-error',
  MODEL_DOWNLOAD_CANCELLED: 'model-download-cancelled',

  // Hotkey
  HOTKEY_PRESSED:  'hotkey-pressed',
  HOTKEY_RELEASED: 'hotkey-released',

  // Transcription
  TRANSCRIPT_NEW:           'transcript:new',
  TRANSCRIPTION_COMPLETE:   'transcription-complete',
  TRANSCRIPTION_ERROR:      'transcription-error',

  // Dictionary
  DICTIONARY_UPDATED: 'dictionary:updated',

  // Auth
  AUTH_READY:           'auth:ready',
  AUTH_UNAUTHENTICATED: 'auth:unauthenticated',
} as const

export type AppEvent = typeof EVENTS[keyof typeof EVENTS]
