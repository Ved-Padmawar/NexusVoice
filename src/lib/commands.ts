/**
 * Centralized Tauri command name constants.
 * Use these instead of raw string literals so renames are caught at compile time.
 */
export const COMMANDS = {
  // Auth
  LOGIN_WITH_TOKENS:    'login_with_tokens',
  REGISTER_WITH_TOKENS: 'register_with_tokens',
  STORE_REFRESH_TOKEN:  'store_refresh_token',
  CLEAR_STORED_TOKEN:   'clear_stored_token',
  GET_AUTH_STATE:       'get_auth_state',
  GET_CURRENT_USER:     'get_current_user',
  REFRESH_TOKEN:        'refresh_token',

  // Transcription
  START_TRANSCRIPTION: 'start_transcription',
  STOP_TRANSCRIPTION:  'stop_transcription',
  TYPE_TEXT:           'type_text',

  // Hotkey
  REGISTER_HOTKEY:     'register_hotkey',
  UNREGISTER_HOTKEY:   'unregister_hotkey',
  GET_REGISTERED_HOTKEYS: 'get_registered_hotkeys',

  // Data
  GET_TRANSCRIPTS:        'get_transcripts',
  SAVE_TRANSCRIPT:        'save_transcript',
  GET_USAGE_STATS:        'get_usage_stats',
  GET_DICTIONARY:         'get_dictionary',
  UPDATE_DICTIONARY:      'update_dictionary',
  DELETE_DICTIONARY_ENTRY:'delete_dictionary_entry',
  SEARCH_TRANSCRIPTS:     'search_transcripts',

  // Model
  GET_MODEL_INFO:        'get_model_info',
  GET_HARDWARE_PROFILE:  'get_hardware_profile',
  SET_MODEL_OVERRIDE:    'set_model_override',
  CLEAR_MODEL_OVERRIDE:  'clear_model_override',
  RETRY_MODEL_DOWNLOAD:  'retry_model_download',

  // System
  OPEN_LOGS_FOLDER: 'open_logs_folder',
} as const

export type AppCommand = typeof COMMANDS[keyof typeof COMMANDS]
