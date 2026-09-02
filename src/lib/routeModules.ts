import { ROUTES } from './routes'
import { createElement, lazy, type ComponentType } from 'react'

// A warmed route renders synchronously. Passing an already-loaded module through
// a new React.lazy promise still suspends once and can hold its fallback for 300ms.
function preloadable(load: () => Promise<{ default: ComponentType }>) {
  let loaded: ComponentType | undefined
  let pending: ReturnType<typeof load> | undefined
  const preload = () => pending ??= load().then(module => {
    loaded = module.default
    return module
  }).catch(error => { pending = undefined; throw error })
  const Lazy = lazy(preload)
  function Page() { return createElement(loaded ?? Lazy) }
  return { Page, preload }
}

const settings = preloadable(() => import('../pages/Settings').then(m => ({ default: m.Settings })))
const dictionary = preloadable(() => import('../pages/Dictionary').then(m => ({ default: m.Dictionary })))
export const SettingsPage = settings.Page
export const DictionaryPage = dictionary.Page

export function preloadRoute(path: string) {
  if (path === ROUTES.SETTINGS) void settings.preload().catch(() => {})
  if (path === ROUTES.DICTIONARY) void dictionary.preload().catch(() => {})
}
