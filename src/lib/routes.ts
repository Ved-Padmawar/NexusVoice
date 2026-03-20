export const ROUTES = {
  DASHBOARD: '/',
  DICTIONARY: '/dictionary',
  SETTINGS: '/settings',
  AUTH: '/auth',
} as const

export type AppRoute = typeof ROUTES[keyof typeof ROUTES]

export const SETTINGS_TABS = {
  GENERAL: 'general',
  ABOUT: 'about',
} as const

export type SettingsTab = typeof SETTINGS_TABS[keyof typeof SETTINGS_TABS]
