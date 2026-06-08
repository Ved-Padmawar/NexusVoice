/**
 * Centralized Tauri command name constants.
 * Use these instead of raw string literals so renames are caught at compile time.
 */
export const COMMANDS = {
  // Auth (local-profile model — no tokens)
  LOGIN:                'login',
  REGISTER:             'register',
  LOGOUT:               'logout',
  GET_AUTH_STATE:       'get_auth_state',
  GET_CURRENT_USER:     'get_current_user',

  // Transcription
  START_TRANSCRIPTION: 'start_transcription',
  STOP_TRANSCRIPTION:  'stop_transcription',
  START_DICTATION:     'start_dictation',
  PAUSE_DICTATION:     'pause_dictation',
  RESUME_DICTATION:    'resume_dictation',
  COMMIT_DICTATION:    'commit_dictation',
  TYPE_TEXT:           'type_text',

  // Hotkey
  REGISTER_HOTKEY:     'register_hotkey',
  UNREGISTER_HOTKEY:   'unregister_hotkey',
  REGISTER_DICTATION_HOTKEY:   'register_dictation_hotkey',
  UNREGISTER_DICTATION_HOTKEY: 'unregister_dictation_hotkey',
  REGISTER_DICTATION_COMMIT_HOTKEY:   'register_dictation_commit_hotkey',
  UNREGISTER_DICTATION_COMMIT_HOTKEY: 'unregister_dictation_commit_hotkey',
  GET_REGISTERED_HOTKEYS: 'get_registered_hotkeys',

  // Data
  GET_TRANSCRIPTS:        'get_transcripts',
  SAVE_TRANSCRIPT:        'save_transcript',
  DELETE_TRANSCRIPT:      'delete_transcript',
  EXPORT_TRANSCRIPTS:     'export_transcripts',
  GET_USAGE_STATS:        'get_usage_stats',
  GET_DICTIONARY:         'get_dictionary',
  UPDATE_DICTIONARY:      'update_dictionary',
  DELETE_DICTIONARY_ENTRY:'delete_dictionary_entry',
  SEARCH_TRANSCRIPTS:     'search_transcripts',

  // Model
  GET_MODEL_INFO:          'get_model_info',
  GET_HARDWARE_PROFILE:    'get_hardware_profile',
  SET_MODEL_OVERRIDE:      'set_model_override',
  CLEAR_MODEL_OVERRIDE:    'clear_model_override',
  RETRY_MODEL_DOWNLOAD:    'retry_model_download',
  CANCEL_MODEL_DOWNLOAD:   'cancel_model_download',
  GET_BEAM_SIZE:           'get_beam_size',
  SET_BEAM_SIZE:           'set_beam_size',
  GET_DOWNLOADED_MODELS:   'get_downloaded_models',
  DELETE_MODEL:            'delete_model',

  // Formatting LLM (OpenAI-compatible HTTP endpoint)
  GET_FORMAT_CONFIG:       'get_format_config',
  SET_FORMAT_CONFIG:       'set_format_config',
  TEST_FORMAT_CONNECTION:  'test_format_connection',

  // System
  OPEN_LOGS_FOLDER: 'open_logs_folder',
  LOG_FRONTEND:     'log_frontend',
} as const

export type AppCommand = typeof COMMANDS[keyof typeof COMMANDS]
