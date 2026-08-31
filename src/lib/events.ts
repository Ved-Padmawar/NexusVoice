/**
 * Centralized Tauri event name constants.
 * Use these instead of raw string literals so renames are caught at compile time.
 */
export const EVENTS = {
  // Model download lifecycle
  MODEL_DOWNLOAD_START:     'model-download-start',
  MODEL_DOWNLOAD_RUNNING:   'model-download-running',
  MODEL_DOWNLOAD_PROGRESS:  'model-download-progress',
  MODEL_DOWNLOAD_COMPLETE:  'model-download-complete',
  MODEL_DOWNLOAD_ERROR:     'model-download-error',
  MODEL_DOWNLOAD_CANCELLED: 'model-download-cancelled',
  MODEL_EVICTED:            'model-evicted',
  MODEL_SWITCHED:           'model-switched',
  LANGUAGE_RESET:           'language-reset',

  // Hotkey
  HOTKEY_PRESSED:  'hotkey-pressed',
  HOTKEY_RELEASED: 'hotkey-released',
  DICTATION_HOTKEY_PRESSED: 'dictation-hotkey-pressed',
  DICTATION_COMMIT_HOTKEY_PRESSED: 'dictation-commit-hotkey-pressed',

  // Transcription
  TRANSCRIPT_NEW:           'transcript:new',
  TRANSCRIPTION_COMPLETE:   'transcription-complete',
  TRANSCRIPTION_ERROR:      'transcription-error',

  // Dictionary

  // Auth
  AUTH_READY:           'auth:ready',
  AUTH_UNAUTHENTICATED: 'auth:unauthenticated',

  // Pill
  PILL_THEME_CHANGED: 'pill:theme-changed',
  PILL_WAVEFORM_STYLE_CHANGED: 'pill:waveform-style-changed',
  PILL_WAVEFORM: 'pill:waveform',
} as const

export type AppEvent = typeof EVENTS[keyof typeof EVENTS]
