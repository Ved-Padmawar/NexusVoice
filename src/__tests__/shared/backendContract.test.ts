/**
 * Every call into the backend goes through the generated client in
 * `bindings.ts`, whose command and event names come from the Rust types. A raw
 * `invoke('name')` or `listen('name')` skips that and drifts silently.
 */
import { describe, it, expect } from 'vitest'

// Vite inlines the matched files as text at transform time, so this needs no
// node APIs — and it picks up newly added files automatically.
const sources = import.meta.glob('/src/**/*.{ts,tsx}', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>

const appSources = Object.entries(sources)
  .filter(([path]) => !path.includes('__tests__') && !path.endsWith('/bindings.ts') && !path.includes('/test/'))

function importersOf(module: string, names: RegExp) {
  return appSources
    .filter(([, text]) => [...text.matchAll(new RegExp(`import\\s*{([^}]*)}\\s*from\\s*'${module}'`, 'g'))]
      .some(m => names.test(m[1])))
    .map(([path]) => path)
}

describe('backend contract', () => {
  it('found the app sources to check', () => {
    expect(appSources.length).toBeGreaterThan(20)
  })

  it('never invokes a command by name outside the generated client', () => {
    expect(importersOf('@tauri-apps/api/core', /\binvoke\b/)).toEqual([])
  })

  it('never listens to or emits an event by name outside the generated client', () => {
    expect(importersOf('@tauri-apps/api/event', /\b(listen|once|emit)\b/)).toEqual([])
  })
})
