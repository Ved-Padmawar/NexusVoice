export const ROUTES = {
  DASHBOARD: '/',
  DICTIONARY: '/dictionary',
  SETTINGS: '/settings',
} as const

export const SETTINGS_TABS = {
  GENERAL: 'general',
  VOICE: 'voice',
  APPEARANCE: 'appearance',
  ABOUT: 'about',
} as const

export type SettingsTab = typeof SETTINGS_TABS[keyof typeof SETTINGS_TABS]
